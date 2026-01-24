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
