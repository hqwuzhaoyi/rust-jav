//! Time-limited, revalidated plans for permanent filesystem deletion.
//!
//! Discovery is restricted to administrator-approved Hard-Link Search Roots.
//! Related hard links are reported, but are never silently promoted to approved
//! deletion paths.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use rustix::fs::{
    fsync, open, openat, renameat_with, statat, unlinkat, AtFlags, FileType as RustixFileType,
    Mode, OFlags, RenameFlags,
};

use crate::management_tasks::TaskStore;

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
    secure_path: PathBuf,
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
    secure_hard_link_search_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSnapshot {
    device: u64,
    inode: u64,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    allocated_size: u64,
    link_count: u64,
}

impl FileSnapshot {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            allocated_size: metadata.blocks().saturating_mul(512),
            link_count: metadata.nlink(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanExecutionError {
    Expired,
    Persistence(String),
}

impl fmt::Display for PlanExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expired => write!(formatter, "Operation Plan has expired"),
            Self::Persistence(message) => write!(formatter, "{message}"),
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
    approved_roots: Vec<PathBuf>,
    hard_link_search_roots: Vec<PathBuf>,
}

impl PermanentDeletionPlanner {
    pub fn new(hard_link_search_roots: Vec<PathBuf>) -> Self {
        Self {
            approved_roots: hard_link_search_roots.clone(),
            hard_link_search_roots,
        }
    }

    pub fn with_roots(approved_roots: Vec<PathBuf>, hard_link_search_roots: Vec<PathBuf>) -> Self {
        Self {
            approved_roots,
            hard_link_search_roots,
        }
    }

    pub fn create_plan(
        &self,
        approved_paths: Vec<PathBuf>,
        lifetime: Duration,
        now: SystemTime,
    ) -> Result<PermanentDeletionPlan, PlanCreationError> {
        if self.approved_roots.is_empty() || self.hard_link_search_roots.is_empty() {
            return Err(PlanCreationError::NoApprovedRoots);
        }
        if lifetime.is_zero() {
            return Err(PlanCreationError::InvalidLifetime);
        }
        let expires_at = now
            .checked_add(lifetime)
            .ok_or(PlanCreationError::ExpirationOverflow)?;
        let approved_roots = self.validated_roots(&self.approved_roots)?;
        let search_roots = self.validated_roots(&self.hard_link_search_roots)?;
        let secure_hard_link_search_roots = search_roots
            .iter()
            .map(|root| {
                fs::canonicalize(root).map_err(|source| PlanCreationError::InvalidRoot {
                    path: root.clone(),
                    source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut planned_paths = Vec::with_capacity(approved_paths.len());
        let mut seen_paths = HashSet::new();

        for path in approved_paths {
            let absolute =
                absolute_path(&path).map_err(|source| PlanCreationError::InspectPath {
                    path: path.clone(),
                    source,
                })?;
            validate_path_scope(&absolute, &approved_roots)?;
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
        for root in &search_roots {
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
            hard_link_search_roots: search_roots,
            approved_paths: planned_paths,
            related_hard_links,
            logical_size,
            reclaimable_space,
            video_warnings,
            secure_hard_link_search_roots,
        })
    }

    pub fn execute(
        &self,
        plan: &PermanentDeletionPlan,
        now: SystemTime,
        tasks: &TaskStore,
        task_id: &str,
    ) -> Result<DeletionExecutionResult, PlanExecutionError> {
        self.execute_with_hooks(plan, now, tasks, task_id, |_| {}, |_, _| {})
    }

    #[doc(hidden)]
    pub fn execute_with_pre_unlink_hook<F>(
        &self,
        plan: &PermanentDeletionPlan,
        now: SystemTime,
        tasks: &TaskStore,
        task_id: &str,
        mut before_unlink: F,
    ) -> Result<DeletionExecutionResult, PlanExecutionError>
    where
        F: FnMut(&PlannedDeletionPath),
    {
        self.execute_with_hooks(plan, now, tasks, task_id, &mut before_unlink, |_, _| {})
    }

    #[doc(hidden)]
    pub fn execute_journaled_with_capture_hook<Hook>(
        &self,
        plan: &PermanentDeletionPlan,
        now: SystemTime,
        tasks: &TaskStore,
        task_id: &str,
        mut after_capture: Hook,
    ) -> Result<DeletionExecutionResult, PlanExecutionError>
    where
        Hook: FnMut(&PlannedDeletionPath, &str),
    {
        self.execute_with_hooks(plan, now, tasks, task_id, |_| {}, &mut after_capture)
    }

    fn execute_with_hooks<Before, Captured>(
        &self,
        plan: &PermanentDeletionPlan,
        now: SystemTime,
        tasks: &TaskStore,
        task_id: &str,
        mut before_unlink: Before,
        mut after_capture: Captured,
    ) -> Result<DeletionExecutionResult, PlanExecutionError>
    where
        Before: FnMut(&PlannedDeletionPath),
        Captured: FnMut(&PlannedDeletionPath, &str),
    {
        validate_durable_authority(tasks, task_id, plan)?;
        let validations = preflight_plan(plan, now)?;
        let approved_roots = open_approved_root_handles(plan).map_err(|error| {
            PlanExecutionError::Persistence(format!(
                "cannot open approved deletion roots without following symlinks: {error}"
            ))
        })?;
        let mut outcomes = Vec::with_capacity(plan.approved_paths.len());
        for (path, invalid) in plan.approved_paths.iter().zip(validations) {
            let source = path.path.to_str().ok_or_else(|| {
                PlanExecutionError::Persistence(
                    "approved path is not UTF-8 and cannot be durably journaled".to_string(),
                )
            })?;
            let journal = tasks
                .start_deletion_item(task_id, source, path.identity.device, path.identity.inode)
                .map_err(|error| {
                    PlanExecutionError::Persistence(format!(
                        "cannot create durable deletion journal before mutation: {error}"
                    ))
                })?;
            let outcome = if let Some(invalid) = invalid {
                invalid
            } else {
                before_unlink(path);
                let mut captured = || {
                    tasks
                        .mark_deletion_item_quarantined(task_id, journal.id)
                        .map_err(|error| error.to_string())?;
                    after_capture(path, &journal.quarantine_token);
                    Ok(())
                };
                let mut advance_phase = |expected: &str, next: &str| {
                    tasks
                        .advance_deletion_item_phase(task_id, journal.id, expected, next)
                        .map_err(|error| error.to_string())
                };
                remove_path_atomically(
                    path,
                    &journal.quarantine_token,
                    &approved_roots,
                    &mut captured,
                    &mut advance_phase,
                )
                .map_err(|error| {
                    PlanExecutionError::Persistence(format!(
                        "durable deletion filesystem phase for {} is unresolved: {error}",
                        path.path.display()
                    ))
                })?
            };
            let status = match outcome.status {
                DeletionOutcomeStatus::Deleted => "deleted",
                DeletionOutcomeStatus::Changed => "changed",
                DeletionOutcomeStatus::Failed => "failed",
            };
            tasks
                .complete_item(
                    task_id,
                    journal.id,
                    status,
                    outcome.message.as_deref(),
                )
                .map_err(|error| {
                PlanExecutionError::Persistence(format!(
                    "cannot update durable deletion journal after filesystem outcome for {}: {error}",
                    path.path.display()
                ))
            })?;
            outcomes.push(outcome);
        }
        Ok(deletion_result(outcomes))
    }

    fn validated_roots(
        &self,
        configured_roots: &[PathBuf],
    ) -> Result<Vec<PathBuf>, PlanCreationError> {
        let mut roots = Vec::with_capacity(configured_roots.len());
        for root in configured_roots {
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

fn validate_durable_authority(
    tasks: &TaskStore,
    task_id: &str,
    plan: &PermanentDeletionPlan,
) -> Result<(), PlanExecutionError> {
    let task = tasks
        .get(task_id)
        .map_err(|error| {
            PlanExecutionError::Persistence(format!(
                "cannot load durable deletion authority: {error}"
            ))
        })?
        .ok_or_else(|| {
            PlanExecutionError::Persistence("durable deletion task does not exist".to_string())
        })?;
    let authority = task.operation_plan.ok_or_else(|| {
        PlanExecutionError::Persistence(
            "durable deletion task has no Operation Plan authority snapshot".to_string(),
        )
    })?;
    let expires_at = plan
        .expires_at
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs());
    let roots_match = authority["hard_link_search_roots"]
        .as_array()
        .is_some_and(|roots| {
            roots.len() == plan.hard_link_search_roots.len()
                && roots
                    .iter()
                    .zip(&plan.hard_link_search_roots)
                    .all(|(stored, planned)| stored.as_str() == planned.to_str())
        });
    let paths_match = authority["paths"].as_array().is_some_and(|paths| {
        paths.len() == plan.approved_paths.len()
            && paths
                .iter()
                .zip(&plan.approved_paths)
                .all(|(stored, planned)| {
                    stored["path"].as_str() == planned.path.to_str()
                        && stored["filesystem_identity"]["device"].as_u64()
                            == Some(planned.identity.device)
                        && stored["filesystem_identity"]["inode"].as_u64()
                            == Some(planned.identity.inode)
                })
    });
    if authority["expires_at"].as_u64() != expires_at || !roots_match || !paths_match {
        return Err(PlanExecutionError::Persistence(
            "durable deletion task authority does not match the supplied Operation Plan"
                .to_string(),
        ));
    }
    Ok(())
}

fn preflight_plan(
    plan: &PermanentDeletionPlan,
    now: SystemTime,
) -> Result<Vec<Option<DeletionOutcome>>, PlanExecutionError> {
    if now > plan.expires_at {
        return Err(PlanExecutionError::Expired);
    }
    // Revalidate the complete plan before the first unlink. Otherwise the
    // first approved hard link would legitimately change link counts for
    // later approved paths and make a unified deletion invalidate itself.
    Ok(plan
        .approved_paths
        .iter()
        .map(revalidate_path)
        .collect::<Vec<_>>())
}

struct ApprovedRootHandle {
    path: PathBuf,
    directory: OwnedFd,
}

fn open_approved_root_handles(
    plan: &PermanentDeletionPlan,
) -> Result<Vec<ApprovedRootHandle>, String> {
    plan.hard_link_search_roots
        .iter()
        .zip(&plan.secure_hard_link_search_roots)
        .map(|(display_root, secure_root)| {
            open_absolute_directory_without_symlinks(secure_root)
                .map(|directory| ApprovedRootHandle {
                    path: secure_root.clone(),
                    directory,
                })
                .map_err(|error| format!("{}: {error}", display_root.display()))
        })
        .collect()
}

fn open_absolute_directory_without_symlinks(path: &Path) -> Result<OwnedFd, String> {
    if !path.is_absolute() {
        return Err(format!("approved root is not absolute: {}", path.display()));
    }
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
    let mut current = open(Path::new("/"), flags, Mode::empty())
        .map_err(|error| format!("cannot open filesystem root: {error}"))?;
    for component in path.components() {
        match component {
            std::path::Component::RootDir => {}
            std::path::Component::Normal(component) => {
                current = openat(&current, component, flags, Mode::empty()).map_err(|error| {
                    format!(
                        "cannot open approved root component {} without following symlinks: {error}",
                        component.to_string_lossy()
                    )
                })?;
            }
            _ => {
                return Err(format!(
                    "approved root is not a normalized absolute path: {}",
                    path.display()
                ))
            }
        }
    }
    Ok(current)
}

fn secure_parent_handle(path: &Path, roots: &[ApprovedRootHandle]) -> Result<OwnedFd, String> {
    let root = roots
        .iter()
        .filter(|root| path.starts_with(&root.path))
        .max_by_key(|root| root.path.components().count())
        .ok_or_else(|| format!("{} is outside opened approved roots", path.display()))?;
    let relative = path
        .strip_prefix(&root.path)
        .map_err(|_| "approved path cannot be made relative to its opened root".to_string())?;
    let parent = relative
        .parent()
        .ok_or_else(|| "approved path has no relative parent".to_string())?;
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
    let mut current = openat(&root.directory, ".", flags, Mode::empty())
        .map_err(|error| format!("cannot duplicate approved root handle: {error}"))?;
    for component in parent.components() {
        let std::path::Component::Normal(component) = component else {
            return Err("approved path parent is not normalized".to_string());
        };
        current = openat(&current, component, flags, Mode::empty()).map_err(|error| {
            format!(
                "cannot open approved parent component {} without following symlinks: {error}",
                component.to_string_lossy()
            )
        })?;
    }
    Ok(current)
}

fn deletion_result(outcomes: Vec<DeletionOutcome>) -> DeletionExecutionResult {
    let deleted = outcomes
        .iter()
        .filter(|outcome| outcome.status == DeletionOutcomeStatus::Deleted)
        .count();
    let partial = deleted > 0 && deleted < outcomes.len();
    DeletionExecutionResult {
        outcomes,
        partial,
        rolled_back: false,
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

fn planned_path(path: PathBuf, metadata: fs::Metadata) -> io::Result<PlannedDeletionPath> {
    let file_type = classify_file_type(&metadata);
    let parent = path.parent().unwrap_or(Path::new("/"));
    let secure_parent = fs::canonicalize(parent)?;
    let secure_path = path
        .file_name()
        .map(|name| secure_parent.join(name))
        .unwrap_or(secure_parent);
    Ok(PlannedDeletionPath {
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
        secure_path,
        snapshot: FileSnapshot::from_metadata(&metadata),
    })
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
    result.push(
        planned_path(path.to_path_buf(), metadata).map_err(|source| {
            PlanCreationError::InspectPath {
                path: path.to_path_buf(),
                source,
            }
        })?,
    );
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

fn revalidate_path(path: &PlannedDeletionPath) -> Option<DeletionOutcome> {
    let metadata = match fs::symlink_metadata(&path.path) {
        Ok(metadata) => metadata,
        Err(error) => {
            return Some(DeletionOutcome {
                path: path.path.clone(),
                status: DeletionOutcomeStatus::Changed,
                message: Some(format!(
                    "filesystem identity cannot be revalidated: {error}"
                )),
            })
        }
    };
    let current = FileSnapshot::from_metadata(&metadata);
    let changed = if path.file_type == FileType::Directory {
        current.device != path.snapshot.device || current.inode != path.snapshot.inode
    } else {
        current != path.snapshot
    };
    if changed {
        return Some(DeletionOutcome {
            path: path.path.clone(),
            status: DeletionOutcomeStatus::Changed,
            message: Some(
                "device, inode, size, modification time, allocation, or link count changed"
                    .to_string(),
            ),
        });
    }

    None
}

fn remove_path_atomically(
    path: &PlannedDeletionPath,
    quarantine_token: &str,
    approved_roots: &[ApprovedRootHandle],
    after_capture: &mut impl FnMut() -> Result<(), String>,
    advance_phase: &mut impl FnMut(&str, &str) -> Result<(), String>,
) -> Result<DeletionOutcome, String> {
    let Some(parent) = path.path.parent() else {
        return Ok(failed_outcome(
            path,
            "approved path has no parent directory".to_string(),
        ));
    };
    let Some(name) = path.path.file_name() else {
        return Ok(failed_outcome(
            path,
            "approved path has no final component".to_string(),
        ));
    };
    let parent_fd = match secure_parent_handle(&path.secure_path, approved_roots) {
        Ok(parent_fd) => parent_fd,
        Err(error) => {
            return Ok(failed_outcome(
                path,
                format!("cannot open parent directory from approved root handle: {error}"),
            ))
        }
    };
    let quarantine_name = match durable_quarantine_name(quarantine_token) {
        Ok(name) => name,
        Err(error) => return Ok(failed_outcome(path, error)),
    };
    if let Err(error) = renameat_with(
        &parent_fd,
        name,
        &parent_fd,
        quarantine_name,
        RenameFlags::NOREPLACE,
    ) {
        let message =
            format!("pathname could not be atomically quarantined before unlink: {error}");
        return Ok(if error == rustix::io::Errno::NOENT {
            changed_outcome(path, message)
        } else {
            failed_outcome(path, message)
        });
    }
    fsync(&parent_fd).map_err(|error| {
        format!(
            "captured durable quarantine {} but parent directory fsync failed: {error}",
            parent.join(quarantine_name).display()
        )
    })?;
    after_capture()?;

    let quarantined = match statat(&parent_fd, quarantine_name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(metadata) => metadata,
        Err(error) => {
            return Err(format!(
                "durable quarantine {} cannot be inspected safely: {error}",
                parent.join(quarantine_name).display()
            ))
        }
    };
    if !quarantined_snapshot_matches(path, &quarantined) {
        let quarantine_path = parent.join(quarantine_name);
        advance_phase("quarantined", "restoring_replacement")?;
        let message = match renameat_with(
            &parent_fd,
            quarantine_name,
            &parent_fd,
            name,
            RenameFlags::NOREPLACE,
        ) {
            Ok(()) => {
                fsync(&parent_fd).map_err(|error| {
                    format!(
                        "replacement was restored after identity mismatch but parent directory fsync failed: {error}"
                    )
                })?;
                advance_phase("restoring_replacement", "restored")?;
                "filesystem identity changed after preflight; replacement was restored".to_string()
            }
            Err(error) => format!(
                "filesystem identity changed after preflight; replacement was preserved at {} because its pathname could not be restored without overwriting another entry: {error}",
                quarantine_path.display()
            ),
        };
        return Ok(changed_outcome(path, message));
    }

    let flags = if path.file_type == FileType::Directory {
        AtFlags::REMOVEDIR
    } else {
        AtFlags::empty()
    };
    advance_phase("quarantined", "unlinking")?;
    match unlinkat(&parent_fd, quarantine_name, flags) {
        Ok(()) => {
            fsync(&parent_fd).map_err(|error| {
                format!(
                    "approved inode was unlinked but parent directory fsync failed; journal remains quarantined for recovery: {error}"
                )
            })?;
            advance_phase("unlinking", "unlinked")?;
            Ok(DeletionOutcome {
                path: path.path.clone(),
                status: DeletionOutcomeStatus::Deleted,
                message: None,
            })
        }
        Err(error) => {
            let quarantine_path = parent.join(quarantine_name);
            // Persist rollback intent before moving the durable quarantine.
            // If this write fails, leave the quarantine in place so restart
            // recovery has an unambiguous locator and filesystem state.
            advance_phase("unlinking", "restoring_replacement")?;
            let message = match renameat_with(
                &parent_fd,
                quarantine_name,
                &parent_fd,
                name,
                RenameFlags::NOREPLACE,
            ) {
                Ok(()) => {
                    fsync(&parent_fd).map_err(|sync_error| {
                        format!(
                            "approved inode rollback completed after unlink failure but parent directory fsync failed: {sync_error}"
                        )
                    })?;
                    advance_phase("restoring_replacement", "restored")?;
                    format!(
                        "approved inode could not be unlinked and was restored to its source path: {error}"
                    )
                }
                Err(rollback) => format!(
                    "approved inode could not be unlinked: {error}; rollback refused without replacement: {rollback}; durable quarantine remains at {}",
                    quarantine_path.display()
                ),
            };
            Ok(failed_outcome(path, message))
        }
    }
}

fn quarantined_snapshot_matches(path: &PlannedDeletionPath, metadata: &rustix::fs::Stat) -> bool {
    if metadata.st_dev as u64 != path.snapshot.device || metadata.st_ino != path.snapshot.inode {
        return false;
    }
    if path.file_type == FileType::Directory {
        return RustixFileType::from_raw_mode(metadata.st_mode) == RustixFileType::Directory;
    }
    let file_type_matches = match path.file_type {
        FileType::RegularFile => {
            RustixFileType::from_raw_mode(metadata.st_mode) == RustixFileType::RegularFile
        }
        FileType::Symlink => {
            RustixFileType::from_raw_mode(metadata.st_mode) == RustixFileType::Symlink
        }
        FileType::Other => true,
        FileType::Directory => unreachable!(),
    };
    file_type_matches
        && metadata.st_size as u64 == path.snapshot.size
        && metadata.st_mtime == path.snapshot.modified_seconds
        && metadata.st_mtime_nsec == path.snapshot.modified_nanoseconds
        && (metadata.st_blocks as u64).saturating_mul(512) == path.snapshot.allocated_size
}

fn durable_quarantine_name(token: &str) -> Result<&str, String> {
    let suffix = token
        .strip_prefix(".rust-jav-quarantine-item-")
        .ok_or_else(|| "invalid durable deletion quarantine token prefix".to_string())?;
    if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("invalid durable deletion quarantine token id".to_string());
    }
    Ok(token)
}

fn changed_outcome(path: &PlannedDeletionPath, message: String) -> DeletionOutcome {
    DeletionOutcome {
        path: path.path.clone(),
        status: DeletionOutcomeStatus::Changed,
        message: Some(message),
    }
}

fn failed_outcome(path: &PlannedDeletionPath, message: String) -> DeletionOutcome {
    DeletionOutcome {
        path: path.path.clone(),
        status: DeletionOutcomeStatus::Failed,
        message: Some(message),
    }
}
