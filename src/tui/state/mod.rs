//! TUI State module
//!
//! Contains state management structures for the TUI application.

pub mod exec_state;
pub mod file_state;
pub mod log_persistence;
pub mod op_state;

pub use exec_state::{ExecutionProgress, FileOperationResult};
pub use file_state::{
    ConflictResolution, FileNode, FileStatus, LogEntry, LogLevel, MoveConflict, MoveTarget,
    PathCompleter, TreeState,
};
pub use log_persistence::{LogPersistence, SessionStats};
pub use op_state::{Operation, OperationType, OperationsState};

use std::path::PathBuf;

use crossterm::event::KeyEvent;

/// Actions that can be sent through the action channel.
#[derive(Debug, Clone)]
pub enum Action {
    /// Regular tick for animations/updates
    Tick,

    /// Application should quit
    Quit,

    /// Force UI redraw
    Render,

    /// Key press event
    KeyPress(KeyEvent),

    /// Terminal resize event
    Resize { width: u16, height: u16 },

    /// Change focused panel
    FocusPanel(crate::tui::app::Panel),

    /// Cycle to next panel
    FocusNext,

    /// Cycle to previous panel
    FocusPrevious,

    /// Directory scan completed
    ScanComplete { root: PathBuf, nodes: Vec<FileNode> },

    /// Directory scan failed
    ScanFailed { root: PathBuf, error: String },

    /// Lazy load of directory children completed
    ChildrenLoaded {
        parent: PathBuf,
        children: Vec<FileNode>,
    },

    /// Toggle an operation's enabled state
    ToggleOperation(usize),

    /// Toggle all operations
    ToggleAllOperations,

    /// Start execution
    ExecutionStart { total_files: u64 },

    /// Progress update during execution
    ProgressUpdate {
        processed: u64,
        current_file: PathBuf,
        current_operation: String,
    },

    /// Single file operation completed
    FileOperationComplete {
        file: PathBuf,
        result: Result<(), String>,
    },

    /// Execution completed
    ExecutionComplete,

    /// Execution cancelled
    ExecutionCancelled,

    /// Show confirmation dialog
    ShowConfirmDialog { title: String, message: String },

    /// Show move dialog
    ShowMoveDialog { files: Vec<PathBuf> },

    /// Dialog confirmed
    DialogConfirm,

    /// Dialog cancelled
    DialogCancel,

    /// Add log entry
    Log(LogEntry),

    /// Enter search mode
    EnterSearch,

    /// Exit search mode
    ExitSearch,

    /// Show help panel
    ShowHelp,

    /// Hide help panel
    HideHelp,
}
