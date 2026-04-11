//! Operation executor module
//!
//! Bridges TUI operations with file_utils functions.

use regex::Regex;
use std::path::{Path, PathBuf};

use crate::file_utils::ad_patterns;

use super::state::{Operation, OperationType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedAction {
    pub kind: String,
    pub source: Option<PathBuf>,
    pub target: Option<PathBuf>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationPlan {
    pub op_type: OperationType,
    pub actions: Vec<PlannedAction>,
    pub warnings: Vec<String>,
}

/// Result of executing an operation
#[derive(Debug, Clone)]
pub struct OperationResult {
    /// Type of operation that was executed
    pub op_type: OperationType,
    /// Whether the operation succeeded
    pub success: bool,
    /// Number of files affected
    pub affected_count: usize,
    /// Error message if failed
    pub error: Option<String>,
    /// List of affected file paths
    pub affected_files: Vec<PathBuf>,
    /// List of failed file paths with error messages (T100)
    pub failed_files: Vec<(PathBuf, String)>,
}

impl OperationResult {
    /// Create a successful result
    pub fn success(op_type: OperationType, affected_files: Vec<PathBuf>) -> Self {
        let affected_count = affected_files.len();
        Self {
            op_type,
            success: true,
            affected_count,
            error: None,
            affected_files,
            failed_files: Vec::new(),
        }
    }

    /// Create a failed result
    pub fn failure(op_type: OperationType, error: String) -> Self {
        Self {
            op_type,
            success: false,
            affected_count: 0,
            error: Some(error),
            affected_files: Vec::new(),
            failed_files: Vec::new(),
        }
    }

    /// Create a partial success result with some failures (T100)
    pub fn partial(
        op_type: OperationType,
        affected_files: Vec<PathBuf>,
        failed_files: Vec<(PathBuf, String)>,
    ) -> Self {
        let affected_count = affected_files.len();
        let has_failures = !failed_files.is_empty();
        Self {
            op_type,
            success: !has_failures || affected_count > 0,
            affected_count,
            error: if has_failures {
                Some(format!("{} file(s) failed", failed_files.len()))
            } else {
                None
            },
            affected_files,
            failed_files,
        }
    }
}

/// Operation executor that runs file operations
pub struct OperationExecutor {
    /// Source directory to operate on
    source_dir: PathBuf,
    /// Whether to run in dry-run mode (simulation)
    dry_run: bool,
}

impl OperationExecutor {
    /// Create a new executor
    pub fn new(source_dir: PathBuf, dry_run: bool) -> Self {
        Self {
            source_dir,
            dry_run,
        }
    }

    /// Analyze all operations and return affected file counts
    /// This is used to populate the Operations panel with statistics
    pub async fn analyze_operations(&self) -> Vec<(OperationType, Vec<PathBuf>)> {
        let mut results = Vec::new();

        for op_type in OperationType::all() {
            let affected_files = match op_type {
                OperationType::DeleteAdFiles => self.find_ad_files().await,
                OperationType::OrganizeByCode => self.find_files_with_jav_codes().await,
                OperationType::CleanEmptyDirs => self.find_empty_directories().await,
                OperationType::StandardizeNames => self.find_files_to_standardize().await,
                OperationType::ExtractCodes => self.find_files_with_jav_codes().await,
                OperationType::CategorizeFiles => self.find_files_to_categorize().await,
                OperationType::MoveOrigin => self.find_origin_files().await,
                OperationType::RemoveDuplicates => self.find_duplicate_files().await,
            };
            results.push((op_type, affected_files));
        }

        results
    }

    pub async fn plan_operation(&self, op_type: OperationType) -> OperationPlan {
        match op_type {
            OperationType::DeleteAdFiles => self.plan_delete_ad_files().await,
            OperationType::OrganizeByCode => self.plan_organize_by_code().await,
            OperationType::CleanEmptyDirs => self.plan_clean_empty_dirs().await,
            OperationType::StandardizeNames => self.plan_standardize_names().await,
            OperationType::ExtractCodes => self.plan_extract_codes().await,
            OperationType::CategorizeFiles => self.plan_categorize_files().await,
            OperationType::MoveOrigin => self.plan_move_origin().await,
            OperationType::RemoveDuplicates => self.plan_remove_duplicates().await,
        }
    }

    /// Execute a single operation
    pub async fn execute(&self, operation: &Operation) -> OperationResult {
        if self.dry_run {
            self.simulate_operation(operation).await
        } else {
            self.run_operation(operation).await
        }
    }

    /// Simulate an operation (dry-run mode)
    async fn simulate_operation(&self, operation: &Operation) -> OperationResult {
        // In simulation mode, we scan for affected files without modifying them
        match operation.op_type {
            OperationType::DeleteAdFiles => {
                let files = self.find_ad_files().await;
                OperationResult::success(operation.op_type, files)
            }
            OperationType::OrganizeByCode => {
                let files = self.find_files_with_jav_codes().await;
                OperationResult::success(operation.op_type, files)
            }
            OperationType::CleanEmptyDirs => {
                let dirs = self.find_empty_directories().await;
                OperationResult::success(operation.op_type, dirs)
            }
            OperationType::StandardizeNames => {
                let files = self.find_files_to_standardize().await;
                OperationResult::success(operation.op_type, files)
            }
            OperationType::ExtractCodes => {
                let files = self.find_files_with_jav_codes().await;
                OperationResult::success(operation.op_type, files)
            }
            OperationType::CategorizeFiles => {
                let files = self.find_files_to_categorize().await;
                OperationResult::success(operation.op_type, files)
            }
            OperationType::MoveOrigin => {
                let files = self.find_origin_files().await;
                OperationResult::success(operation.op_type, files)
            }
            OperationType::RemoveDuplicates => {
                let files = self.find_duplicate_files().await;
                OperationResult::success(operation.op_type, files)
            }
        }
    }

    /// Run an operation (modifies files)
    async fn run_operation(&self, operation: &Operation) -> OperationResult {
        match operation.op_type {
            OperationType::DeleteAdFiles => self.execute_delete_ad_files().await,
            OperationType::OrganizeByCode => self.execute_organize_by_code().await,
            OperationType::CleanEmptyDirs => self.execute_clean_empty_dirs().await,
            OperationType::StandardizeNames => self.execute_standardize_names().await,
            OperationType::ExtractCodes => self.execute_extract_codes().await,
            OperationType::CategorizeFiles => self.execute_categorize_files().await,
            OperationType::MoveOrigin => self.execute_move_origin().await,
            OperationType::RemoveDuplicates => self.execute_remove_duplicates().await,
        }
    }

    // === Pattern Matching helpers (T047-T048 implementation) ===

    /// Find files that match JAV code patterns
    /// JAV codes typically follow patterns like: ABC-123, ABCD-123, AB-1234
    async fn find_files_with_jav_codes(&self) -> Vec<PathBuf> {
        let jav_pattern = Regex::new(r"(?i)[A-Z]{2,6}[-_]?\d{2,5}").unwrap();
        let mut matched_files = Vec::new();

        for file in Self::collect_video_files_sync(&self.source_dir) {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                if jav_pattern.is_match(name) {
                    matched_files.push(file);
                }
            }
        }
        matched_files
    }

    /// Find files that need categorization (Chinese subtitles or Uncensored)
    async fn find_files_to_categorize(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();

        for file in Self::collect_video_files_sync(&self.source_dir) {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                let name_upper = name.to_uppercase();
                // Chinese subtitle patterns: -C, -ch, CH, C_X1080X
                let is_chinese = name_upper.contains("-C.")
                    || name_upper.contains("-C-")
                    || name_upper.ends_with("-C")
                    || name.to_lowercase().ends_with("-ch")
                    || name.to_lowercase().contains("-ch.")
                    || name_upper.contains("C_X1080X");

                // Uncensored patterns: -UC, UNCENSORED
                let is_uncensored = name_upper.contains("-UC") || name_upper.contains("UNCENSORED");

                if is_chinese || is_uncensored {
                    files.push(file);
                }
            }
        }
        files
    }

    /// Find regular files (not CHINESE or UNCENSORED) for ORIGIN folder
    async fn find_origin_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();

        for file in Self::collect_video_files_sync(&self.source_dir) {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                let name_upper = name.to_uppercase();
                // Chinese subtitle patterns
                let is_chinese = name_upper.contains("-C.")
                    || name_upper.contains("-C-")
                    || name_upper.ends_with("-C")
                    || name.to_lowercase().ends_with("-ch")
                    || name.to_lowercase().contains("-ch.")
                    || name_upper.contains("C_X1080X");

                // Uncensored patterns
                let is_uncensored = name_upper.contains("-UC") || name_upper.contains("UNCENSORED");

                // Regular files: not Chinese and not Uncensored
                if !is_chinese && !is_uncensored {
                    files.push(file);
                }
            }
        }
        files
    }

    async fn find_empty_directories(&self) -> Vec<PathBuf> {
        let mut empty_dirs = Vec::new();
        Self::find_empty_dirs_recursive(&self.source_dir, &mut empty_dirs);
        empty_dirs
    }

    fn find_empty_dirs_recursive(dir: &Path, result: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Recursively check subdirectories first
                    Self::find_empty_dirs_recursive(&path, result);
                    // Then check if this directory is empty
                    if Self::is_empty_dir(&path) {
                        result.push(path);
                    }
                }
            }
        }
    }

    async fn find_files_to_standardize(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let prefixes = self.get_prefixes();

        Self::find_files_with_prefixes_recursive(&self.source_dir, &prefixes, &mut files);
        files
    }

    fn find_files_with_prefixes_recursive(
        dir: &Path,
        prefixes: &[String],
        result: &mut Vec<PathBuf>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if prefixes.iter().any(|p| name.starts_with(p)) {
                            result.push(path);
                        }
                    }
                } else if path.is_dir() {
                    Self::find_files_with_prefixes_recursive(&path, prefixes, result);
                }
            }
        }
    }

    async fn find_duplicate_files(&self) -> Vec<PathBuf> {
        // Simple size-based duplicate detection
        use std::collections::HashMap;
        let mut size_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();

        for file in Self::collect_video_files_sync(&self.source_dir) {
            if let Ok(metadata) = std::fs::metadata(&file) {
                let size = metadata.len();
                // Only consider files > 1MB as potential duplicates
                if size > 1_000_000 {
                    size_map.entry(size).or_default().push(file);
                }
            }
        }

        // Return files that have the same size (potential duplicates)
        let mut duplicates = Vec::new();
        for (_, files) in size_map {
            if files.len() > 1 {
                // Skip the first one, add rest as potential duplicates
                duplicates.extend(files.into_iter().skip(1));
            }
        }
        duplicates
    }

    // === Ad-file helpers ===

    /// Return the list of ad-patterns loaded from the embedded patterns.txt.
    fn ad_patterns() -> Vec<String> {
        if let Some(guard) = crate::config::get_config() {
            guard.patterns.clone()
        } else {
            // Fall back to the embedded static patterns when global config is not initialised.
            ad_patterns::embedded_patterns()
        }
    }

    /// Walk `source_dir` recursively and collect every file whose name matches at least one
    /// ad-pattern.  Video files are **not** excluded — spec decision #6.
    async fn find_ad_files(&self) -> Vec<PathBuf> {
        let patterns = Self::ad_patterns();
        let regexes = ad_patterns::compile_patterns(&patterns);

        let mut matches = Vec::new();
        Self::walk_files_for_ad(&self.source_dir, &regexes, &mut matches);
        matches
    }

    fn walk_files_for_ad(dir: &Path, regexes: &[Regex], out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }

            let path = entry.path();
            if file_type.is_dir() {
                // Skip hidden dirs, build artefacts, and known non-user directories.
                let skip = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with('.') || n == "target")
                    .unwrap_or(false);
                if !skip {
                    Self::walk_files_for_ad(&path, regexes, out);
                }
            } else if file_type.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if ad_patterns::filename_matches_any_compiled(name, regexes) {
                        out.push(path);
                    }
                }
            }
        }
    }

    async fn plan_delete_ad_files(&self) -> OperationPlan {
        let files = self.find_ad_files().await;
        let mut warnings = Vec::new();
        if files.iter().any(|f| {
            f.extension()
                .and_then(|e| e.to_str())
                .map(|e| matches!(e.to_lowercase().as_str(), "mp4" | "mkv" | "avi" | "wmv"))
                .unwrap_or(false)
        }) {
            warnings
                .push("some matched ad files are video files — review before applying".to_string());
        }
        OperationPlan {
            op_type: OperationType::DeleteAdFiles,
            actions: files
                .into_iter()
                .map(|file| PlannedAction {
                    kind: "delete-file".to_string(),
                    source: Some(file),
                    target: None,
                    reason: Some("filename matches ad/spam pattern".to_string()),
                })
                .collect(),
            warnings,
        }
    }

    async fn execute_delete_ad_files(&self) -> OperationResult {
        let files = self.find_ad_files().await;
        let mut affected = Vec::new();
        let mut failed = Vec::new();

        for file in files {
            match std::fs::remove_file(&file) {
                Ok(_) => affected.push(file),
                Err(e) => failed.push((file, e.to_string())),
            }
        }

        OperationResult::partial(OperationType::DeleteAdFiles, affected, failed)
    }

    // === End ad-file helpers ===

    async fn plan_organize_by_code(&self) -> OperationPlan {
        let files = self.find_files_with_jav_codes().await;
        let jav_pattern = Regex::new(r"(?i)([A-Z]{2,6})[-_]?(\d{2,5})").unwrap();
        let mut actions = Vec::new();
        let mut warnings = Vec::new();

        for file in files {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                if let Some(captures) = jav_pattern.captures(name) {
                    let code = format!(
                        "{}-{}",
                        captures
                            .get(1)
                            .map(|m| m.as_str().to_uppercase())
                            .unwrap_or_default(),
                        captures.get(2).map(|m| m.as_str()).unwrap_or_default()
                    );
                    let target = self.source_dir.join(&code).join(file.file_name().unwrap());
                    actions.push(PlannedAction {
                        kind: "move".to_string(),
                        source: Some(file),
                        target: Some(target),
                        reason: Some(format!("organize into code directory {code}")),
                    });
                } else {
                    warnings.push(format!("Could not derive code for {}", file.display()));
                }
            }
        }

        OperationPlan {
            op_type: OperationType::OrganizeByCode,
            actions,
            warnings,
        }
    }

    async fn plan_clean_empty_dirs(&self) -> OperationPlan {
        let dirs = self.find_empty_directories().await;
        OperationPlan {
            op_type: OperationType::CleanEmptyDirs,
            actions: dirs
                .into_iter()
                .map(|dir| PlannedAction {
                    kind: "delete-dir".to_string(),
                    source: Some(dir),
                    target: None,
                    reason: None,
                })
                .collect(),
            warnings: Vec::new(),
        }
    }

    async fn plan_standardize_names(&self) -> OperationPlan {
        let files = self.find_files_to_standardize().await;
        let prefixes = self.get_prefixes();
        let mut actions = Vec::new();

        for file in files {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                if let Some(prefix) = prefixes
                    .iter()
                    .find(|prefix| name.starts_with(prefix.as_str()))
                {
                    let target = file.with_file_name(name.replacen(prefix, "", 1));
                    actions.push(PlannedAction {
                        kind: "rename".to_string(),
                        source: Some(file),
                        target: Some(target),
                        reason: Some("remove known prefix".to_string()),
                    });
                }
            }
        }

        OperationPlan {
            op_type: OperationType::StandardizeNames,
            actions,
            warnings: Vec::new(),
        }
    }

    async fn plan_extract_codes(&self) -> OperationPlan {
        let files = self.find_files_with_jav_codes().await;
        let mut actions = Vec::new();

        for file in files {
            if let Some((target, code)) = Self::extract_code_target(&file) {
                actions.push(PlannedAction {
                    kind: "extract-code".to_string(),
                    source: Some(file),
                    target: Some(target),
                    reason: Some(format!(
                        "normalize filename around detected JAV code {code}"
                    )),
                });
            }
        }

        OperationPlan {
            op_type: OperationType::ExtractCodes,
            actions,
            warnings: Vec::new(),
        }
    }

    async fn plan_categorize_files(&self) -> OperationPlan {
        let files = self.find_files_to_categorize().await;
        let mut actions = Vec::new();

        for file in files {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                let target_dir = if Self::is_uncensored_name(name) {
                    self.source_dir.join("UNCENSORED")
                } else {
                    self.source_dir.join("CHINESE")
                };
                actions.push(PlannedAction {
                    kind: "move".to_string(),
                    source: Some(file.clone()),
                    target: Some(target_dir.join(file.file_name().unwrap())),
                    reason: Some("categorize by filename suffix".to_string()),
                });
            }
        }

        OperationPlan {
            op_type: OperationType::CategorizeFiles,
            actions,
            warnings: Vec::new(),
        }
    }

    async fn plan_move_origin(&self) -> OperationPlan {
        let files = self.find_origin_files().await;
        let origin_dir = self.source_dir.join("ORIGIN");
        OperationPlan {
            op_type: OperationType::MoveOrigin,
            actions: files
                .into_iter()
                .map(|file| PlannedAction {
                    kind: "move".to_string(),
                    source: Some(file.clone()),
                    target: Some(origin_dir.join(file.file_name().unwrap())),
                    reason: Some("move uncategorized video into ORIGIN".to_string()),
                })
                .collect(),
            warnings: Vec::new(),
        }
    }

    async fn plan_remove_duplicates(&self) -> OperationPlan {
        let files = self.find_duplicate_files().await;
        let mut warnings = Vec::new();
        if !files.is_empty() {
            warnings.push(
                "duplicate detection is size-based only; review preview carefully before apply"
                    .to_string(),
            );
        }

        OperationPlan {
            op_type: OperationType::RemoveDuplicates,
            actions: files
                .into_iter()
                .map(|file| PlannedAction {
                    kind: "delete-file".to_string(),
                    source: Some(file),
                    target: None,
                    reason: Some("potential duplicate selected by size heuristic".to_string()),
                })
                .collect(),
            warnings,
        }
    }

    // === Execution helpers (actually modify files) ===

    async fn execute_organize_by_code(&self) -> OperationResult {
        let files = self.find_files_with_jav_codes().await;
        let jav_pattern = Regex::new(r"(?i)([A-Z]{2,6})[-_]?(\d{2,5})").unwrap();
        let mut affected = Vec::new();
        let mut failed = Vec::new(); // T100: Track failed files

        for file in files {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                if let Some(captures) = jav_pattern.captures(name) {
                    let code = format!(
                        "{}-{}",
                        captures
                            .get(1)
                            .map(|m| m.as_str().to_uppercase())
                            .unwrap_or_default(),
                        captures.get(2).map(|m| m.as_str()).unwrap_or_default()
                    );
                    let target_dir = self.source_dir.join(&code);

                    // Create directory if needed
                    if !target_dir.exists() {
                        if let Err(e) = std::fs::create_dir_all(&target_dir) {
                            failed.push((file.clone(), format!("Cannot create dir: {}", e)));
                            continue;
                        }
                    }

                    let target_file = target_dir.join(file.file_name().unwrap());
                    match std::fs::rename(&file, &target_file) {
                        Ok(_) => affected.push(target_file),
                        Err(e) => failed.push((file.clone(), e.to_string())),
                    }
                }
            }
        }

        OperationResult::partial(OperationType::OrganizeByCode, affected, failed)
    }

    async fn execute_clean_empty_dirs(&self) -> OperationResult {
        let dirs = self.find_empty_directories().await;
        let mut affected = Vec::new();
        let mut failed = Vec::new(); // T100: Track failed dirs

        // Sort by path length descending to remove deepest directories first
        let mut dirs = dirs;
        dirs.sort_by_key(|path| std::cmp::Reverse(path.to_string_lossy().len()));

        for dir in dirs {
            match std::fs::remove_dir(&dir) {
                Ok(_) => affected.push(dir),
                Err(e) => failed.push((dir, e.to_string())),
            }
        }

        OperationResult::partial(OperationType::CleanEmptyDirs, affected, failed)
    }

    async fn execute_standardize_names(&self) -> OperationResult {
        let files = self.find_files_to_standardize().await;
        let prefixes = self.get_prefixes();
        let mut affected = Vec::new();
        let mut failed = Vec::new(); // T100

        for file in files {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                for prefix in &prefixes {
                    if name.starts_with(prefix) {
                        let new_name = name.replacen(prefix, "", 1);
                        let new_path = file.with_file_name(new_name);
                        match std::fs::rename(&file, &new_path) {
                            Ok(_) => affected.push(new_path),
                            Err(e) => failed.push((file.clone(), e.to_string())),
                        }
                        break;
                    }
                }
            }
        }

        OperationResult::partial(OperationType::StandardizeNames, affected, failed)
    }

    async fn execute_extract_codes(&self) -> OperationResult {
        let files = self.find_files_with_jav_codes().await;
        let mut affected = Vec::new();
        let mut failed = Vec::new();

        for file in files {
            let Some((target, _code)) = Self::extract_code_target(&file) else {
                continue;
            };

            if target.exists() && target != file {
                failed.push((
                    file.clone(),
                    format!("target already exists: {}", target.display()),
                ));
                continue;
            }

            match std::fs::rename(&file, &target) {
                Ok(_) => affected.push(target),
                Err(e) => failed.push((file.clone(), e.to_string())),
            }
        }

        OperationResult::partial(OperationType::ExtractCodes, affected, failed)
    }

    async fn execute_categorize_files(&self) -> OperationResult {
        let files = self.find_files_to_categorize().await;
        let mut affected = Vec::new();
        let mut failed = Vec::new(); // T100

        let chinese_dir = self.source_dir.join("CHINESE");
        let uncensored_dir = self.source_dir.join("UNCENSORED");

        for file in files {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                let name_upper = name.to_uppercase();

                let target_dir = if name_upper.contains("-UC") || name_upper.contains("UNCENSORED")
                {
                    &uncensored_dir
                } else {
                    &chinese_dir
                };

                if !target_dir.exists() {
                    if let Err(e) = std::fs::create_dir_all(target_dir) {
                        failed.push((file.clone(), format!("Cannot create dir: {}", e)));
                        continue;
                    }
                }

                let target_file = target_dir.join(file.file_name().unwrap());
                if !target_file.exists() {
                    match std::fs::rename(&file, &target_file) {
                        Ok(_) => affected.push(target_file),
                        Err(e) => failed.push((file.clone(), e.to_string())),
                    }
                }
            }
        }

        OperationResult::partial(OperationType::CategorizeFiles, affected, failed)
    }

    async fn execute_move_origin(&self) -> OperationResult {
        let files = self.find_origin_files().await;
        let mut affected = Vec::new();
        let mut failed = Vec::new(); // T100

        let origin_dir = self.source_dir.join("ORIGIN");

        for file in files {
            if !origin_dir.exists() {
                if let Err(e) = std::fs::create_dir_all(&origin_dir) {
                    failed.push((file.clone(), format!("Cannot create dir: {}", e)));
                    continue;
                }
            }

            if let Some(file_name) = file.file_name() {
                let target_file = origin_dir.join(file_name);
                if !target_file.exists() {
                    match std::fs::rename(&file, &target_file) {
                        Ok(_) => affected.push(target_file),
                        Err(e) => failed.push((file.clone(), e.to_string())),
                    }
                }
            }
        }

        OperationResult::partial(OperationType::MoveOrigin, affected, failed)
    }

    async fn execute_remove_duplicates(&self) -> OperationResult {
        let files = self.find_duplicate_files().await;
        let mut affected = Vec::new();
        let mut failed = Vec::new(); // T100

        // Only remove files in dry_run=false mode
        for file in files {
            match std::fs::remove_file(&file) {
                Ok(_) => affected.push(file),
                Err(e) => failed.push((file, e.to_string())),
            }
        }

        OperationResult::partial(OperationType::RemoveDuplicates, affected, failed)
    }

    // === Utility functions ===

    fn is_empty_dir(path: &Path) -> bool {
        match std::fs::read_dir(path) {
            Ok(mut entries) => entries.next().is_none(),
            Err(_) => false,
        }
    }

    fn collect_video_files_sync(dir: &Path) -> Vec<PathBuf> {
        let video_extensions = [
            "mp4", "mkv", "avi", "wmv", "mov", "flv", "webm", "rmvb", "rm",
        ];
        let mut files = Vec::new();

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if video_extensions.contains(&ext.to_lowercase().as_str()) {
                            files.push(path);
                        }
                    }
                } else if path.is_dir() {
                    // Recursively collect from subdirectories
                    let sub_files = Self::collect_video_files_sync(&path);
                    files.extend(sub_files);
                }
            }
        }

        files
    }

    fn get_prefixes(&self) -> Vec<String> {
        // Get prefixes from config or use defaults
        if let Some(guard) = crate::config::get_config() {
            guard.prefixes.clone()
        } else {
            vec![
                "[7sht.me]@".to_string(),
                "hhd800.com@".to_string(),
                "zzpp01.com@".to_string(),
                "[98t.tv]@".to_string(),
                "[ThZu.Cc]@".to_string(),
                "[99u.me]@".to_string(),
                "[22sht.me]@".to_string(),
                "AVAV66.XYZ@".to_string(),
                "4k2.com@".to_string(),
            ]
        }
    }

    fn is_uncensored_name(name: &str) -> bool {
        let upper = name.to_uppercase();
        upper.contains("-UC") || upper.contains("UNCENSORED")
    }

    fn extract_code_target(file: &Path) -> Option<(PathBuf, String)> {
        let stem = file.file_stem()?.to_str()?;
        let extension = file.extension().and_then(|ext| ext.to_str());
        let jav_pattern = Regex::new(r"(?i)([A-Z]{2,6})[-_]?(\d{2,5})").unwrap();
        let captures = jav_pattern.captures(stem)?;
        let full_match = captures.get(0)?;
        let code = format!(
            "{}-{}",
            captures.get(1)?.as_str().to_uppercase(),
            captures.get(2)?.as_str()
        );
        let suffix = &stem[full_match.end()..];
        let new_stem = format!("{code}{suffix}");
        let new_name = match extension {
            Some(ext) => format!("{new_stem}.{ext}"),
            None => new_stem,
        };
        let target = file.with_file_name(new_name);

        if target == file {
            None
        } else {
            Some((target, code))
        }
    }
}

/// Execute all enabled operations in sequence
pub async fn execute_operations(
    source_dir: PathBuf,
    operations: &[Operation],
    dry_run: bool,
    mut progress_callback: impl FnMut(usize, usize, &OperationResult),
) -> Vec<OperationResult> {
    let executor = OperationExecutor::new(source_dir, dry_run);
    let enabled_ops: Vec<_> = operations.iter().filter(|op| op.enabled).collect();
    let total = enabled_ops.len();
    let mut results = Vec::new();

    for (idx, operation) in enabled_ops.iter().enumerate() {
        let result = executor.execute(operation).await;
        progress_callback(idx + 1, total, &result);
        results.push(result);
    }

    results
}
