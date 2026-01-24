//! Operation state management
//!
//! Contains data structures for batch operations in the TUI.

use std::path::PathBuf;

/// Types of batch operations available
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    /// Organize files into folders by code
    OrganizeByCode,
    /// Clean up empty directories
    CleanEmptyDirs,
    /// Rename files to standard format
    StandardizeNames,
    /// Extract codes from filenames
    ExtractCodes,
    /// Move files to categorized folders
    CategorizeFiles,
    /// Remove duplicate files
    RemoveDuplicates,
}

impl OperationType {
    /// Get human-readable name for the operation
    pub fn name(&self) -> &'static str {
        match self {
            OperationType::OrganizeByCode => "Organize by Code",
            OperationType::CleanEmptyDirs => "Clean Empty Directories",
            OperationType::StandardizeNames => "Standardize Names",
            OperationType::ExtractCodes => "Extract Codes",
            OperationType::CategorizeFiles => "Categorize Files",
            OperationType::RemoveDuplicates => "Remove Duplicates",
        }
    }

    /// Get a short description of what this operation does
    pub fn description(&self) -> &'static str {
        match self {
            OperationType::OrganizeByCode => "Move files into folders named by their JAV code",
            OperationType::CleanEmptyDirs => "Remove directories that contain no files",
            OperationType::StandardizeNames => "Rename files to follow standard naming conventions",
            OperationType::ExtractCodes => "Extract JAV codes from filenames",
            OperationType::CategorizeFiles => "Sort files into category-based folders",
            OperationType::RemoveDuplicates => "Find and remove duplicate files",
        }
    }

    /// Get all available operation types
    pub fn all() -> Vec<OperationType> {
        vec![
            OperationType::OrganizeByCode,
            OperationType::CleanEmptyDirs,
            OperationType::StandardizeNames,
            OperationType::ExtractCodes,
            OperationType::CategorizeFiles,
            OperationType::RemoveDuplicates,
        ]
    }
}

/// Represents a single operation item in the operations list
#[derive(Debug, Clone)]
pub struct Operation {
    /// Type of operation
    pub op_type: OperationType,
    /// Whether this operation is enabled
    pub enabled: bool,
    /// Number of files that would be affected
    pub affected_count: usize,
    /// Total size of affected files in bytes
    pub affected_size: u64,
    /// List of affected file paths
    pub affected_files: Vec<PathBuf>,
}

impl Operation {
    /// Create a new operation
    pub fn new(op_type: OperationType) -> Self {
        Self {
            op_type,
            enabled: false,
            affected_count: 0,
            affected_size: 0,
            affected_files: Vec::new(),
        }
    }

    /// Toggle the enabled state
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// Set the affected files for this operation
    pub fn set_affected(&mut self, files: Vec<PathBuf>) {
        self.affected_count = files.len();
        // Calculate total size of affected files
        self.affected_size = files.iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();
        self.affected_files = files;
    }
}

/// State for the operations list
#[derive(Debug, Clone)]
pub struct OperationsState {
    /// List of operations
    pub operations: Vec<Operation>,
    /// Currently selected operation index
    pub selected: usize,
}

impl Default for OperationsState {
    fn default() -> Self {
        Self::new()
    }
}

impl OperationsState {
    /// Create a new operations state with all operation types
    pub fn new() -> Self {
        let operations = OperationType::all()
            .into_iter()
            .map(Operation::new)
            .collect();
        Self {
            operations,
            selected: 0,
        }
    }

    /// Select the next operation
    pub fn next(&mut self) {
        if !self.operations.is_empty() {
            self.selected = (self.selected + 1) % self.operations.len();
        }
    }

    /// Select the previous operation
    pub fn previous(&mut self) {
        if !self.operations.is_empty() {
            self.selected = self.selected.checked_sub(1).unwrap_or(self.operations.len() - 1);
        }
    }

    /// Toggle the currently selected operation
    pub fn toggle_current(&mut self) {
        if let Some(op) = self.operations.get_mut(self.selected) {
            op.toggle();
        }
    }

    /// Toggle all operations
    pub fn toggle_all(&mut self) {
        let all_enabled = self.operations.iter().all(|op| op.enabled);
        for op in &mut self.operations {
            op.enabled = !all_enabled;
        }
    }

    /// Get the currently selected operation
    pub fn current(&self) -> Option<&Operation> {
        self.operations.get(self.selected)
    }

    /// Check if any operations are enabled
    pub fn has_enabled(&self) -> bool {
        self.operations.iter().any(|op| op.enabled)
    }

    /// Get all enabled operations
    pub fn enabled_operations(&self) -> Vec<&Operation> {
        self.operations.iter().filter(|op| op.enabled).collect()
    }

    /// Count enabled operations
    pub fn enabled_count(&self) -> usize {
        self.operations.iter().filter(|op| op.enabled).count()
    }
}
