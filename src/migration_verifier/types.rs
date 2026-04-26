use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MigrationScope {
    Source,
    ActorsRoot,
}

impl MigrationScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::ActorsRoot => "actors_root",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationActionKind {
    Move,
    Rename,
    DeleteFile,
    HardLink,
}

impl MigrationActionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Rename => "rename",
            Self::DeleteFile => "delete_file",
            Self::HardLink => "hard_link",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationStatus {
    Ok,
    Mismatch,
    Error,
}

impl VerificationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Mismatch => "mismatch",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalStatus {
    AutoPass,
    ManualConfirmRequired,
    Blocked,
}

impl ApprovalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AutoPass => "auto_pass",
            Self::ManualConfirmRequired => "manual_confirm_required",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    pub entry_id: String,
    pub scope: MigrationScope,
    pub relative_path: String,
    pub file_name: String,
    pub extension: String,
    pub size: u64,
    pub file_identity: Option<String>,
    pub origin_before_entry_id: Option<String>,
    pub origin_before_relative_path: Option<String>,
    pub action_ids: Vec<String>,
    pub link_type: String,
    pub link_source_entry_id: Option<String>,
}

impl ManifestEntry {
    pub fn from_scanned(
        entry_id: String,
        scope: MigrationScope,
        relative_path: String,
        size: u64,
    ) -> Self {
        let file_name = Path::new(&relative_path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(relative_path.as_str())
            .to_string();
        let extension = file_extension(&file_name);
        Self {
            entry_id: entry_id.clone(),
            scope,
            relative_path: relative_path.clone(),
            file_name,
            extension,
            size,
            file_identity: None,
            origin_before_entry_id: Some(entry_id.clone()),
            origin_before_relative_path: Some(relative_path),
            action_ids: Vec::new(),
            link_type: "none".to_string(),
            link_source_entry_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationAction {
    pub action_id: String,
    pub kind: MigrationActionKind,
    pub scope: MigrationScope,
    pub source: Option<PathBuf>,
    pub target: Option<PathBuf>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeConfig {
    pub scope: MigrationScope,
    pub root_dir: PathBuf,
    pub allow_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationPlan {
    pub command: String,
    pub mode: String,
    pub scopes: Vec<ScopeConfig>,
    pub actions: Vec<MigrationAction>,
    pub requires_manual_confirm: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeCountSummary {
    pub scope: MigrationScope,
    pub before_count: usize,
    pub expected_count: usize,
    pub after_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationSummary {
    pub verification_status: VerificationStatus,
    pub approval_status: ApprovalStatus,
    pub exit_code: i32,
    pub report_path: Option<PathBuf>,
    pub scopes: Vec<ScopeCountSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeManifest {
    pub scope: MigrationScope,
    pub root_dir: PathBuf,
    pub entries: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeDiff {
    pub scope: MigrationScope,
    pub missing_files: Vec<String>,
    pub unexpected_files: Vec<String>,
    pub mismatched_files: Vec<MismatchedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MismatchedFile {
    pub relative_path: String,
    pub mismatch_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeExtensionCounts {
    pub scope: MigrationScope,
    pub before: Vec<(String, usize)>,
    pub expected: Vec<(String, usize)>,
    pub after: Vec<(String, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedStats {
    pub expected_new_links: usize,
    pub expected_existing_links: usize,
    pub plan_conflicts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationReport {
    pub version: u32,
    pub command: String,
    pub mode: String,
    pub verification_status: VerificationStatus,
    pub approval_status: ApprovalStatus,
    pub exit_code: i32,
    pub report_path: PathBuf,
    pub before: Vec<ScopeManifest>,
    pub expected: Vec<ScopeManifest>,
    pub after: Vec<ScopeManifest>,
    pub scope_counts: Vec<ScopeCountSummary>,
    pub scope_extension_counts: Vec<ScopeExtensionCounts>,
    pub diffs: Vec<ScopeDiff>,
    pub failed_actions: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub expected_stats: ExpectedStats,
}

pub fn file_extension(file_name: &str) -> String {
    let path = Path::new(file_name);
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_else(|| "[no_ext]".to_string())
}
