// File Tree Component API Contract
// Defines the public interface for the file tree panel

use std::path::{Path, PathBuf};

/// Public API for FileTreeComponent
impl FileTreeComponent {
    // === Construction ===

    /// Create a new file tree component for the given root directory.
    pub fn new(root: PathBuf) -> Self;

    /// Create with pre-loaded nodes (for testing).
    pub fn with_nodes(nodes: Vec<FileNode>) -> Self;

    // === Navigation ===

    /// Move selection to next item.
    pub fn next(&mut self);

    /// Move selection to previous item.
    pub fn previous(&mut self);

    /// Move selection to first item.
    pub fn first(&mut self);

    /// Move selection to last item.
    pub fn last(&mut self);

    /// Page down (move by viewport height).
    pub fn page_down(&mut self, viewport_height: usize);

    /// Page up (move by viewport height).
    pub fn page_up(&mut self, viewport_height: usize);

    // === Expansion ===

    /// Expand the currently selected directory.
    /// Returns Ok(()) if expanded, Err if not a directory or already expanded.
    pub async fn expand_selected(&mut self) -> Result<(), TreeError>;

    /// Collapse the currently selected directory.
    /// If a file is selected, collapse its parent.
    pub fn collapse_selected(&mut self);

    /// Expand all directories (use with caution for large trees).
    pub async fn expand_all(&mut self) -> Result<(), TreeError>;

    /// Collapse all directories.
    pub fn collapse_all(&mut self);

    // === Selection ===

    /// Get the currently selected node.
    pub fn selected(&self) -> Option<&FileNode>;

    /// Get the currently selected node (mutable).
    pub fn selected_mut(&mut self) -> Option<&mut FileNode>;

    /// Get the path of the currently selected item.
    pub fn selected_path(&self) -> Option<&Path>;

    /// Enter multi-select mode.
    pub fn enter_multi_select(&mut self);

    /// Exit multi-select mode.
    pub fn exit_multi_select(&mut self);

    /// Toggle selection of current item in multi-select mode.
    pub fn toggle_multi_select(&mut self);

    /// Get all multi-selected paths.
    pub fn multi_selected_paths(&self) -> Vec<&Path>;

    // === Search ===

    /// Enter search mode.
    pub fn enter_search(&mut self);

    /// Update search query.
    pub fn update_search(&mut self, query: &str);

    /// Exit search mode.
    pub fn exit_search(&mut self);

    /// Jump to next search match.
    pub fn next_match(&mut self);

    /// Jump to previous search match.
    pub fn prev_match(&mut self);

    // === State Queries ===

    /// Check if a path is expanded.
    pub fn is_expanded(&self, path: &Path) -> bool;

    /// Check if multi-select mode is active.
    pub fn is_multi_select(&self) -> bool;

    /// Check if search mode is active.
    pub fn is_searching(&self) -> bool;

    /// Get total number of visible items.
    pub fn visible_count(&self) -> usize;

    // === Operations ===

    /// Mark selected node(s) for a pending operation.
    pub fn mark_pending(&mut self, op: OperationType);

    /// Clear pending operation from selected node(s).
    pub fn clear_pending(&mut self);

    /// Refresh tree from filesystem.
    pub async fn refresh(&mut self) -> Result<(), TreeError>;
}

/// Errors that can occur in file tree operations.
pub enum TreeError {
    /// Path does not exist.
    PathNotFound(PathBuf),
    /// Permission denied.
    PermissionDenied(PathBuf),
    /// IO error.
    IoError(std::io::Error),
    /// Not a directory.
    NotADirectory(PathBuf),
}
