//! Operation executor module
//!
//! Bridges TUI operations with file_utils functions.

use std::path::{Path, PathBuf};
use regex::Regex;

use super::state::{Operation, OperationType};

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
        Self { source_dir, dry_run }
    }

    /// Analyze all operations and return affected file counts
    /// This is used to populate the Operations panel with statistics
    pub async fn analyze_operations(&self) -> Vec<(OperationType, Vec<PathBuf>)> {
        let mut results = Vec::new();

        for op_type in OperationType::all() {
            let affected_files = match op_type {
                OperationType::OrganizeByCode => self.find_files_with_jav_codes().await,
                OperationType::CleanEmptyDirs => self.find_empty_directories().await,
                OperationType::StandardizeNames => self.find_files_to_standardize().await,
                OperationType::ExtractCodes => self.find_files_with_jav_codes().await,
                OperationType::CategorizeFiles => self.find_files_to_categorize().await,
                OperationType::RemoveDuplicates => self.find_duplicate_files().await,
            };
            results.push((op_type, affected_files));
        }

        results
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
            OperationType::RemoveDuplicates => {
                let files = self.find_duplicate_files().await;
                OperationResult::success(operation.op_type, files)
            }
        }
    }

    /// Run an operation (modifies files)
    async fn run_operation(&self, operation: &Operation) -> OperationResult {
        match operation.op_type {
            OperationType::OrganizeByCode => {
                self.execute_organize_by_code().await
            }
            OperationType::CleanEmptyDirs => {
                self.execute_clean_empty_dirs().await
            }
            OperationType::StandardizeNames => {
                self.execute_standardize_names().await
            }
            OperationType::ExtractCodes => {
                self.execute_extract_codes().await
            }
            OperationType::CategorizeFiles => {
                self.execute_categorize_files().await
            }
            OperationType::RemoveDuplicates => {
                self.execute_remove_duplicates().await
            }
        }
    }

    // === Pattern Matching helpers (T047-T048 implementation) ===

    /// Find files that match JAV code patterns
    /// JAV codes typically follow patterns like: ABC-123, ABCD-123, AB-1234
    async fn find_files_with_jav_codes(&self) -> Vec<PathBuf> {
        let jav_pattern = Regex::new(r"(?i)[A-Z]{2,6}[-_]?\d{2,5}").unwrap();
        let mut matched_files = Vec::new();

        for file in self.collect_video_files_sync(&self.source_dir) {
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

        for file in self.collect_video_files_sync(&self.source_dir) {
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
                let is_uncensored = name_upper.contains("-UC")
                    || name_upper.contains("UNCENSORED");

                if is_chinese || is_uncensored {
                    files.push(file);
                }
            }
        }
        files
    }

    async fn find_empty_directories(&self) -> Vec<PathBuf> {
        let mut empty_dirs = Vec::new();
        self.find_empty_dirs_recursive(&self.source_dir, &mut empty_dirs);
        empty_dirs
    }

    fn find_empty_dirs_recursive(&self, dir: &Path, result: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Recursively check subdirectories first
                    self.find_empty_dirs_recursive(&path, result);
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

        self.find_files_with_prefixes_recursive(&self.source_dir, &prefixes, &mut files);
        files
    }

    fn find_files_with_prefixes_recursive(&self, dir: &Path, prefixes: &[String], result: &mut Vec<PathBuf>) {
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
                    self.find_files_with_prefixes_recursive(&path, prefixes, result);
                }
            }
        }
    }

    async fn find_duplicate_files(&self) -> Vec<PathBuf> {
        // Simple size-based duplicate detection
        use std::collections::HashMap;
        let mut size_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();

        for file in self.collect_video_files_sync(&self.source_dir) {
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

    // === Execution helpers (actually modify files) ===

    async fn execute_organize_by_code(&self) -> OperationResult {
        let files = self.find_files_with_jav_codes().await;
        let jav_pattern = Regex::new(r"(?i)([A-Z]{2,6})[-_]?(\d{2,5})").unwrap();
        let mut affected = Vec::new();

        for file in files {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                if let Some(captures) = jav_pattern.captures(name) {
                    let code = format!("{}-{}",
                        captures.get(1).map(|m| m.as_str().to_uppercase()).unwrap_or_default(),
                        captures.get(2).map(|m| m.as_str()).unwrap_or_default()
                    );
                    let target_dir = self.source_dir.join(&code);

                    // Create directory if needed
                    if !target_dir.exists() {
                        let _ = std::fs::create_dir_all(&target_dir);
                    }

                    let target_file = target_dir.join(file.file_name().unwrap());
                    if std::fs::rename(&file, &target_file).is_ok() {
                        affected.push(target_file);
                    }
                }
            }
        }

        OperationResult::success(OperationType::OrganizeByCode, affected)
    }

    async fn execute_clean_empty_dirs(&self) -> OperationResult {
        let dirs = self.find_empty_directories().await;
        let mut affected = Vec::new();

        // Sort by path length descending to remove deepest directories first
        let mut dirs = dirs;
        dirs.sort_by(|a, b| b.to_string_lossy().len().cmp(&a.to_string_lossy().len()));

        for dir in dirs {
            match std::fs::remove_dir(&dir) {
                Ok(_) => affected.push(dir),
                Err(e) => {
                    return OperationResult::failure(
                        OperationType::CleanEmptyDirs,
                        format!("Failed to remove {}: {}", dir.display(), e),
                    );
                }
            }
        }

        OperationResult::success(OperationType::CleanEmptyDirs, affected)
    }

    async fn execute_standardize_names(&self) -> OperationResult {
        let files = self.find_files_to_standardize().await;
        let prefixes = self.get_prefixes();
        let mut affected = Vec::new();

        for file in files {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                for prefix in &prefixes {
                    if name.starts_with(prefix) {
                        let new_name = name.replacen(prefix, "", 1);
                        let new_path = file.with_file_name(new_name);
                        if std::fs::rename(&file, &new_path).is_ok() {
                            affected.push(new_path);
                        }
                        break;
                    }
                }
            }
        }

        OperationResult::success(OperationType::StandardizeNames, affected)
    }

    async fn execute_extract_codes(&self) -> OperationResult {
        let files = self.find_files_with_jav_codes().await;
        // This operation just identifies files with codes, doesn't modify them
        OperationResult::success(OperationType::ExtractCodes, files)
    }

    async fn execute_categorize_files(&self) -> OperationResult {
        let files = self.find_files_to_categorize().await;
        let mut affected = Vec::new();

        let chinese_dir = self.source_dir.join("CHINESE");
        let uncensored_dir = self.source_dir.join("UNCENSORED");

        for file in files {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                let name_upper = name.to_uppercase();

                let target_dir = if name_upper.contains("-UC") || name_upper.contains("UNCENSORED") {
                    &uncensored_dir
                } else {
                    &chinese_dir
                };

                if !target_dir.exists() {
                    let _ = std::fs::create_dir_all(target_dir);
                }

                let target_file = target_dir.join(file.file_name().unwrap());
                if !target_file.exists() {
                    if std::fs::rename(&file, &target_file).is_ok() {
                        affected.push(target_file);
                    }
                }
            }
        }

        OperationResult::success(OperationType::CategorizeFiles, affected)
    }

    async fn execute_remove_duplicates(&self) -> OperationResult {
        let files = self.find_duplicate_files().await;
        let mut affected = Vec::new();

        // Only remove files in dry_run=false mode
        for file in files {
            if std::fs::remove_file(&file).is_ok() {
                affected.push(file);
            }
        }

        OperationResult::success(OperationType::RemoveDuplicates, affected)
    }

    // === Utility functions ===

    fn is_empty_dir(path: &Path) -> bool {
        match std::fs::read_dir(path) {
            Ok(mut entries) => entries.next().is_none(),
            Err(_) => false,
        }
    }

    fn collect_video_files_sync(&self, dir: &Path) -> Vec<PathBuf> {
        let video_extensions = ["mp4", "mkv", "avi", "wmv", "mov", "flv", "webm", "rmvb", "rm"];
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
                    let sub_files = self.collect_video_files_sync(&path);
                    files.extend(sub_files);
                }
            }
        }

        files
    }

    async fn collect_video_files(&self, dir: &Path) -> Vec<PathBuf> {
        self.collect_video_files_sync(dir)
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
