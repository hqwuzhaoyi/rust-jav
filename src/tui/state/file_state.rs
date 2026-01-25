//! File tree state management
//!
//! Contains data structures for file system representation in the TUI.

use std::collections::HashSet;
use std::path::PathBuf;

/// Status of a file in the tree view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileStatus {
    /// No pending operations
    #[default]
    Unchanged,
    /// File will be moved
    ToMove,
    /// File will be deleted
    ToDelete,
    /// File will be renamed
    ToRename,
    /// Currently selected in multi-select mode
    Selected,
}

/// T050: Represents a move target destination
#[derive(Debug, Clone)]
pub struct MoveTarget {
    /// Target directory path (relative to source_dir)
    pub path: String,
    /// Shortcut key (1-9)
    pub shortcut_key: Option<char>,
    /// Whether this is a preset target
    pub is_preset: bool,
    /// Last used timestamp (for sorting recent targets)
    pub last_used: Option<std::time::SystemTime>,
}

impl MoveTarget {
    /// Create a new preset move target
    pub fn preset(path: &str, shortcut: char) -> Self {
        Self {
            path: path.to_string(),
            shortcut_key: Some(shortcut),
            is_preset: true,
            last_used: None,
        }
    }

    /// Create a custom move target
    pub fn custom(path: &str) -> Self {
        Self {
            path: path.to_string(),
            shortcut_key: None,
            is_preset: false,
            last_used: Some(std::time::SystemTime::now()),
        }
    }

    /// Get default preset targets
    pub fn default_presets() -> Vec<Self> {
        vec![
            Self::preset("CHINESE", '1'),
            Self::preset("UNCENSORED", '2'),
            Self::preset("organized", '3'),
            Self::preset("archive", '4'),
            Self::preset("other", '5'),
        ]
    }
}

/// T059: Result of checking for move conflicts
#[derive(Debug, Clone)]
pub struct MoveConflict {
    /// Source file path
    pub source: PathBuf,
    /// Target file path (already exists)
    pub target: PathBuf,
    /// Size of existing target file
    pub target_size: u64,
    /// Size of source file
    pub source_size: u64,
}

/// T061: How to resolve a move conflict
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolution {
    /// Skip this file, don't move it
    Skip,
    /// Overwrite the existing file
    Overwrite,
    /// Rename the source file with a suffix
    Rename,
}

impl MoveConflict {
    /// Check if moving a file would cause a conflict
    pub fn check(source: &PathBuf, target_dir: &PathBuf) -> Option<Self> {
        if let Some(file_name) = source.file_name() {
            let target = target_dir.join(file_name);
            if target.exists() {
                let target_size = std::fs::metadata(&target)
                    .map(|m| m.len())
                    .unwrap_or(0);
                let source_size = std::fs::metadata(source)
                    .map(|m| m.len())
                    .unwrap_or(0);
                return Some(Self {
                    source: source.clone(),
                    target,
                    target_size,
                    source_size,
                });
            }
        }
        None
    }

    /// Check multiple files for conflicts
    pub fn check_batch(sources: &[PathBuf], target_dir: &PathBuf) -> Vec<Self> {
        sources.iter()
            .filter_map(|s| Self::check(s, target_dir))
            .collect()
    }

    /// T061: Resolve the conflict according to user choice
    pub fn resolve(&self, resolution: ConflictResolution) -> Result<PathBuf, String> {
        match resolution {
            ConflictResolution::Skip => {
                // Don't move, return error to indicate skip
                Err("Skipped".to_string())
            }
            ConflictResolution::Overwrite => {
                // Remove existing file and move
                if let Err(e) = std::fs::remove_file(&self.target) {
                    return Err(format!("Failed to remove existing file: {}", e));
                }
                match std::fs::rename(&self.source, &self.target) {
                    Ok(_) => Ok(self.target.clone()),
                    Err(e) => Err(format!("Failed to move file: {}", e)),
                }
            }
            ConflictResolution::Rename => {
                // Generate a new name with suffix
                let new_target = Self::generate_unique_name(&self.target);
                match std::fs::rename(&self.source, &new_target) {
                    Ok(_) => Ok(new_target),
                    Err(e) => Err(format!("Failed to move file: {}", e)),
                }
            }
        }
    }

    /// Generate a unique filename by adding a numeric suffix
    fn generate_unique_name(path: &PathBuf) -> PathBuf {
        let stem = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e))
            .unwrap_or_default();
        let parent = path.parent().unwrap_or(std::path::Path::new("."));

        let mut counter = 1;
        loop {
            let new_name = format!("{}_{}{}", stem, counter, ext);
            let new_path = parent.join(&new_name);
            if !new_path.exists() {
                return new_path;
            }
            counter += 1;
            if counter > 1000 {
                // Safety limit - use timestamp
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                return parent.join(format!("{}_{}{}", stem, timestamp, ext));
            }
        }
    }
}

/// T057: Path autocomplete helper
pub struct PathCompleter;

impl PathCompleter {
    /// Get completions for a partial path relative to base_dir
    pub fn complete(base_dir: &PathBuf, partial: &str) -> Vec<String> {
        let mut completions = Vec::new();

        // Determine the directory to search and the prefix to match
        let (search_dir, prefix) = if partial.contains('/') || partial.contains('\\') {
            // Has path separator - search in subdirectory
            let path = std::path::Path::new(partial);
            if let Some(parent) = path.parent() {
                let search = base_dir.join(parent);
                let prefix = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                (search, prefix)
            } else {
                (base_dir.clone(), partial.to_lowercase())
            }
        } else {
            // No separator - search in base_dir
            (base_dir.clone(), partial.to_lowercase())
        };

        // Read directory and find matches
        if let Ok(entries) = std::fs::read_dir(&search_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.to_lowercase().starts_with(&prefix) {
                            // Build the completion path
                            let completion = if partial.contains('/') || partial.contains('\\') {
                                let parent = std::path::Path::new(partial)
                                    .parent()
                                    .and_then(|p| p.to_str())
                                    .unwrap_or("");
                                if parent.is_empty() {
                                    name.to_string()
                                } else {
                                    format!("{}/{}", parent, name)
                                }
                            } else {
                                name.to_string()
                            };
                            completions.push(completion);
                        }
                    }
                }
            }
        }

        // Sort completions alphabetically
        completions.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        completions
    }

    /// Get the next completion in a cycle
    pub fn next_completion(completions: &[String], current: &str) -> Option<String> {
        if completions.is_empty() {
            return None;
        }

        // Find current in completions
        if let Some(idx) = completions.iter().position(|c| c == current) {
            // Return next (wrap around)
            let next_idx = (idx + 1) % completions.len();
            Some(completions[next_idx].clone())
        } else {
            // Return first completion
            Some(completions[0].clone())
        }
    }
}

/// Represents a node in the file tree
#[derive(Debug, Clone)]
pub struct FileNode {
    /// File or directory name
    pub name: String,
    /// Full path to the file
    pub path: PathBuf,
    /// Whether this is a directory
    pub is_dir: bool,
    /// Children nodes (for directories)
    pub children: Vec<FileNode>,
    /// Whether children have been loaded (for lazy loading)
    pub children_loaded: bool,
    /// Current status
    pub status: FileStatus,
    /// Depth in tree (for indentation)
    pub depth: usize,
}

impl FileNode {
    /// Create a new file node
    pub fn new(name: String, path: PathBuf, is_dir: bool, depth: usize) -> Self {
        Self {
            name,
            path,
            is_dir,
            children: Vec::new(),
            children_loaded: false,
            status: FileStatus::Unchanged,
            depth,
        }
    }

    /// Check if this node is expanded (has loaded children visible)
    pub fn is_expanded(&self) -> bool {
        self.is_dir && self.children_loaded && !self.children.is_empty()
    }
}

/// State for tree view navigation
#[derive(Debug, Clone, Default)]
pub struct TreeState {
    /// Currently selected index in the flattened tree
    pub selected: usize,
    /// Scroll offset for viewport
    pub offset: usize,
    /// Set of expanded directory paths
    pub expanded: HashSet<PathBuf>,
    /// Set of selected file paths (for multi-select)
    pub selected_files: HashSet<PathBuf>,
    /// Whether multi-select mode is active
    pub multi_select_mode: bool,
}

impl TreeState {
    /// Create a new tree state
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle expansion state of a directory
    pub fn toggle_expanded(&mut self, path: &PathBuf) {
        if self.expanded.contains(path) {
            self.expanded.remove(path);
        } else {
            self.expanded.insert(path.clone());
        }
    }

    /// Check if a directory is expanded
    pub fn is_expanded(&self, path: &PathBuf) -> bool {
        self.expanded.contains(path)
    }

    /// Toggle selection of a file in multi-select mode
    pub fn toggle_selection(&mut self, path: &PathBuf) {
        if self.selected_files.contains(path) {
            self.selected_files.remove(path);
        } else {
            self.selected_files.insert(path.clone());
        }
    }

    /// Check if a file is selected in multi-select mode
    pub fn is_selected(&self, path: &PathBuf) -> bool {
        self.selected_files.contains(path)
    }

    /// Clear all selections
    pub fn clear_selections(&mut self) {
        self.selected_files.clear();
    }
}

/// Log entry for the log viewer
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Timestamp of the entry
    pub timestamp: chrono::DateTime<chrono::Local>,
    /// Log level
    pub level: LogLevel,
    /// Log message
    pub message: String,
    /// Optional file path associated with this entry
    pub file: Option<PathBuf>,
}

impl LogEntry {
    /// Create a new info log entry
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Local::now(),
            level: LogLevel::Info,
            message: message.into(),
            file: None,
        }
    }

    /// Create a new success log entry
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Local::now(),
            level: LogLevel::Success,
            message: message.into(),
            file: None,
        }
    }

    /// Create a new warning log entry
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Local::now(),
            level: LogLevel::Warning,
            message: message.into(),
            file: None,
        }
    }

    /// Create a new error log entry
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Local::now(),
            level: LogLevel::Error,
            message: message.into(),
            file: None,
        }
    }

    /// Set the file path for this entry
    pub fn with_file(mut self, file: PathBuf) -> Self {
        self.file = Some(file);
        self
    }
}

/// Log severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
}
