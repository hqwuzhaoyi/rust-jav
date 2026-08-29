//! Time-limited, revalidated plans for permanent filesystem deletion.
//!
//! Discovery is restricted to administrator-approved Hard-Link Search Roots.
//! Related hard links are reported, but are never silently promoted to approved
//! deletion paths.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    RegularFile,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FilesystemIdentity {
    pub device: u64,
    pub inode: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedDeletionPath {
    pub path: PathBuf,
    pub file_type: FileType,
    pub identity: FilesystemIdentity,
    pub logical_size: u64,
    pub allocated_size: u64,
    pub observed_link_count: u64,
    snapshot: FileSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedHardLink {
    pub path: PathBuf,
    pub identity: FilesystemIdentity,
    pub file_type: FileType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoWarning {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermanentDeletionPlan {
    pub created_at: SystemTime,
    pub expires_at: SystemTime,
    pub hard_link_search_roots: Vec<PathBuf>,
    pub approved_paths: Vec<PlannedDeletionPath>,
    pub related_hard_links: Vec<RelatedHardLink>,
    pub logical_size: u64,
    pub reclaimable_space: u64,
    pub video_warnings: Vec<VideoWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSnapshot {
    device: u64,
    inode: u64,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
}

impl FileSnapshot {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
        }
    }
}

#[derive(Debug)]
pub enum PlanCreationError {
    NoApprovedRoots,
    InvalidLifetime,
    InvalidRoot { path: PathBuf, source: io::Error },
    PathOutsideApprovedRoots(PathBuf),
    InspectPath { path: PathBuf, source: io::Error },
    SearchRoot { path: PathBuf, source: io::Error },
    ExpirationOverflow,
}

impl fmt::Display for PlanCreationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoApprovedRoots => {
                write!(formatter, "at least one Hard-Link Search Root is required")
            }
            Self::InvalidLifetime => write!(
                formatter,
                "Operation Plan lifetime must be greater than zero"
            ),
            Self::InvalidRoot { path, source } => {
                write!(
                    formatter,
                    "invalid Hard-Link Search Root {}: {source}",
                    path.display()
                )
            }
            Self::PathOutsideApprovedRoots(path) => write!(
                formatter,
                "path is outside approved Hard-Link Search Roots: {}",
                path.display()
            ),
            Self::InspectPath { path, source } => {
                write!(
                    formatter,
                    "cannot inspect approved path {}: {source}",
                    path.display()
                )
            }
            Self::SearchRoot { path, source } => {
                write!(
                    formatter,
                    "cannot search Hard-Link Search Root {}: {source}",
                    path.display()
                )
            }
            Self::ExpirationOverflow => write!(formatter, "Operation Plan expiration overflow"),
        }
    }
}

impl std::error::Error for PlanCreationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanExecutionError {
    Expired,
}

impl fmt::Display for PlanExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expired => write!(formatter, "Operation Plan has expired"),
        }
    }
}

impl std::error::Error for PlanExecutionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeletionOutcomeStatus {
    Deleted,
    Changed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionOutcome {
    pub path: PathBuf,
    pub status: DeletionOutcomeStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionExecutionResult {
    pub outcomes: Vec<DeletionOutcome>,
    pub partial: bool,
    /// Permanent deletion has no rollback semantics. This is always false.
    pub rolled_back: bool,
}

#[derive(Debug, Clone)]
pub struct PermanentDeletionPlanner {
    hard_link_search_roots: Vec<PathBuf>,
}

impl PermanentDeletionPlanner {
    pub fn new(hard_link_search_roots: Vec<PathBuf>) -> Self {
        Self {
            hard_link_search_roots,
        }
    }

    pub fn create_plan(
        &self,
        approved_paths: Vec<PathBuf>,
        lifetime: Duration,
        now: SystemTime,
    ) -> Result<PermanentDeletionPlan, PlanCreationError> {
        if self.hard_link_search_roots.is_empty() {
            return Err(PlanCreationError::NoApprovedRoots);
        }
        if lifetime.is_zero() {
            return Err(PlanCreationError::InvalidLifetime);
        }
        let expires_at = now
            .checked_add(lifetime)
            .ok_or(PlanCreationError::ExpirationOverflow)?;
        let roots = self.validated_roots()?;
        let mut planned_paths = Vec::with_capacity(approved_paths.len());
        let mut seen_paths = HashSet::new();

        for path in approved_paths {
            let absolute =
                absolute_path(&path).map_err(|source| PlanCreationError::InspectPath {
                    path: path.clone(),
                    source,
                })?;
            validate_path_scope(&absolute, &roots)?;
            let metadata = fs::symlink_metadata(&absolute).map_err(|source| {
                PlanCreationError::InspectPath {
                    path: absolute.clone(),
                    source,
                }
            })?;
            collect_approved_path(&absolute, metadata, &mut seen_paths, &mut planned_paths)?;
        }

        let approved_by_identity = planned_paths
            .iter()
            .filter(|path| path.file_type == FileType::RegularFile)
            .fold(
                HashMap::<FilesystemIdentity, usize>::new(),
                |mut counts, path| {
                    *counts.entry(path.identity).or_default() += 1;
                    counts
                },
            );
        let approved_path_set = planned_paths
            .iter()
            .map(|path| path.path.clone())
            .collect::<HashSet<_>>();
        let mut related_hard_links = Vec::new();
        for root in &roots {
            search_related_links(
                root,
                &approved_by_identity,
                &approved_path_set,
                &mut related_hard_links,
            )?;
        }
        related_hard_links.sort_by(|left, right| left.path.cmp(&right.path));
        related_hard_links.dedup_by(|left, right| left.path == right.path);

        let mut unique = HashSet::new();
        let logical_size = planned_paths
            .iter()
            .filter(|path| path.file_type == FileType::RegularFile)
            .filter(|path| unique.insert(path.identity))
            .map(|path| path.logical_size)
            .sum();
        let reclaimable_space = reclaimable_space(&planned_paths);
        let video_warnings = planned_paths
            .iter()
            .filter(|path| path.file_type == FileType::RegularFile && is_video(&path.path))
            .map(|path| VideoWarning {
                path: path.path.clone(),
                message: "Permanent deletion removes video content and cannot be rolled back"
                    .to_string(),
            })
            .collect();

        Ok(PermanentDeletionPlan {
            created_at: now,
            expires_at,
            hard_link_search_roots: roots,
            approved_paths: planned_paths,
            related_hard_links,
            logical_size,
            reclaimable_space,
            video_warnings,
        })
    }

    pub fn execute(
        &self,
        plan: &PermanentDeletionPlan,
        now: SystemTime,
    ) -> Result<DeletionExecutionResult, PlanExecutionError> {
        if now > plan.expires_at {
            return Err(PlanExecutionError::Expired);
        }

        let outcomes = plan
            .approved_paths
            .iter()
            .map(execute_path)
            .collect::<Vec<_>>();
        let deleted = outcomes
            .iter()
            .filter(|outcome| outcome.status == DeletionOutcomeStatus::Deleted)
            .count();
        let partial = deleted > 0 && deleted < outcomes.len();

        Ok(DeletionExecutionResult {
            outcomes,
            partial,
            rolled_back: false,
        })
    }

    fn validated_roots(&self) -> Result<Vec<PathBuf>, PlanCreationError> {
        let mut roots = Vec::with_capacity(self.hard_link_search_roots.len());
        for root in &self.hard_link_search_roots {
            let metadata =
                fs::symlink_metadata(root).map_err(|source| PlanCreationError::InvalidRoot {
                    path: root.clone(),
                    source,
                })?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(PlanCreationError::InvalidRoot {
                    path: root.clone(),
                    source: io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "root must be a real directory, not a symlink",
                    ),
                });
            }
            let canonical =
                fs::canonicalize(root).map_err(|source| PlanCreationError::InvalidRoot {
                    path: root.clone(),
                    source,
                })?;
            let traversal_root =
                absolute_path(root).map_err(|source| PlanCreationError::InvalidRoot {
                    path: root.clone(),
                    source,
                })?;
            if !roots
                .iter()
                .any(|existing| fs::canonicalize(existing).ok().as_ref() == Some(&canonical))
            {
                roots.push(traversal_root);
            }
        }
        Ok(roots)
    }
}

fn absolute_path(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn validate_path_scope(path: &Path, roots: &[PathBuf]) -> Result<(), PlanCreationError> {
    // Canonicalize the parent so the final component may safely be a symlink,
    // while intermediate symlink escapes are still rejected.
    let parent = path.parent().unwrap_or(Path::new("/"));
    let canonical_parent =
        fs::canonicalize(parent).map_err(|source| PlanCreationError::InspectPath {
            path: path.to_path_buf(),
            source,
        })?;
    if roots.iter().any(|root| {
        fs::canonicalize(root)
            .map(|canonical_root| canonical_parent.starts_with(canonical_root))
            .unwrap_or(false)
    }) {
        Ok(())
    } else {
        Err(PlanCreationError::PathOutsideApprovedRoots(
            path.to_path_buf(),
        ))
    }
}

fn planned_path(path: PathBuf, metadata: fs::Metadata) -> PlannedDeletionPath {
    let file_type = classify_file_type(&metadata);
    PlannedDeletionPath {
        path,
        file_type,
        identity: FilesystemIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        },
        logical_size: metadata.len(),
        // POSIX st_blocks is expressed in 512-byte units, including on ZFS.
        allocated_size: metadata.blocks().saturating_mul(512),
        observed_link_count: metadata.nlink(),
        snapshot: FileSnapshot::from_metadata(&metadata),
    }
}

fn collect_approved_path(
    path: &Path,
    metadata: fs::Metadata,
    seen_paths: &mut HashSet<PathBuf>,
    result: &mut Vec<PlannedDeletionPath>,
) -> Result<(), PlanCreationError> {
    if !seen_paths.insert(path.to_path_buf()) {
        return Ok(());
    }
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        let entries = fs::read_dir(path).map_err(|source| PlanCreationError::InspectPath {
            path: path.to_path_buf(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| PlanCreationError::InspectPath {
                path: path.to_path_buf(),
                source,
            })?;
            let child = entry.path();
            let child_metadata =
                fs::symlink_metadata(&child).map_err(|source| PlanCreationError::InspectPath {
                    path: child.clone(),
                    source,
                })?;
            collect_approved_path(&child, child_metadata, seen_paths, result)?;
        }
    }
    // Child-first order lets execution remove an approved directory only after
    // every enumerated entry has received its own outcome.
    result.push(planned_path(path.to_path_buf(), metadata));
    Ok(())
}

fn classify_file_type(metadata: &fs::Metadata) -> FileType {
    let kind = metadata.file_type();
    if kind.is_symlink() {
        FileType::Symlink
    } else if kind.is_file() {
        FileType::RegularFile
    } else if kind.is_dir() {
        FileType::Directory
    } else {
        FileType::Other
    }
}

fn search_related_links(
    directory: &Path,
    approved: &HashMap<FilesystemIdentity, usize>,
    approved_paths: &HashSet<PathBuf>,
    result: &mut Vec<RelatedHardLink>,
) -> Result<(), PlanCreationError> {
    let entries = fs::read_dir(directory).map_err(|source| PlanCreationError::SearchRoot {
        path: directory.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| PlanCreationError::SearchRoot {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|source| PlanCreationError::SearchRoot {
                path: path.clone(),
                source,
            })?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            search_related_links(&path, approved, approved_paths, result)?;
        } else if metadata.is_file() {
            let identity = FilesystemIdentity {
                device: metadata.dev(),
                inode: metadata.ino(),
            };
            if approved.contains_key(&identity) && !approved_paths.contains(&path) {
                result.push(RelatedHardLink {
                    path,
                    identity,
                    file_type: FileType::RegularFile,
                });
            }
        }
    }
    Ok(())
}

fn reclaimable_space(paths: &[PlannedDeletionPath]) -> u64 {
    let mut groups: HashMap<FilesystemIdentity, Vec<&PlannedDeletionPath>> = HashMap::new();
    for path in paths
        .iter()
        .filter(|path| matches!(path.file_type, FileType::RegularFile | FileType::Symlink))
    {
        groups.entry(path.identity).or_default().push(path);
    }
    groups
        .values()
        .filter_map(|paths| {
            let first = paths[0];
            (paths.len() as u64 >= first.observed_link_count).then_some(first.allocated_size)
        })
        .sum()
}

fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "mp4" | "mkv" | "avi" | "wmv" | "mov" | "m4v" | "ts" | "webm"
            )
        })
        .unwrap_or(false)
}

fn execute_path(path: &PlannedDeletionPath) -> DeletionOutcome {
    let metadata = match fs::symlink_metadata(&path.path) {
        Ok(metadata) => metadata,
        Err(error) => {
            return DeletionOutcome {
                path: path.path.clone(),
                status: DeletionOutcomeStatus::Changed,
                message: Some(format!(
                    "filesystem identity cannot be revalidated: {error}"
                )),
            }
        }
    };
    let current = FileSnapshot::from_metadata(&metadata);
    let changed = if path.file_type == FileType::Directory {
        current.device != path.snapshot.device || current.inode != path.snapshot.inode
    } else {
        current != path.snapshot
    };
    if changed {
        return DeletionOutcome {
            path: path.path.clone(),
            status: DeletionOutcomeStatus::Changed,
            message: Some("device, inode, size, or modification time changed".to_string()),
        };
    }

    let removal = if path.file_type == FileType::Directory {
        fs::remove_dir(&path.path)
    } else {
        // remove_file unlinks symlinks themselves and never follows their targets.
        fs::remove_file(&path.path)
    };
    match removal {
        Ok(()) => DeletionOutcome {
            path: path.path.clone(),
            status: DeletionOutcomeStatus::Deleted,
            message: None,
        },
        Err(error) => DeletionOutcome {
            path: path.path.clone(),
            status: DeletionOutcomeStatus::Failed,
            message: Some(error.to_string()),
        },
    }
}
