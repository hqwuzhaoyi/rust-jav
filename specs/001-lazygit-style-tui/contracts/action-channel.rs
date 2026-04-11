// Action Channel Contract
// Defines the message types for async communication between TUI and background tasks

use std::path::PathBuf;
use std::time::Instant;

/// Actions that can be sent through the action channel.
/// Used for communication between:
/// - Event loop and application state
/// - Background tasks and UI updates
/// - Components and parent App
pub enum Action {
    // === Lifecycle Events ===

    /// Regular tick for animations/updates (e.g., every 250ms).
    Tick,

    /// Application should quit.
    Quit,

    /// Force UI redraw.
    Render,

    // === User Input Events ===

    /// Key press event from terminal.
    KeyPress(crossterm::event::KeyEvent),

    /// Mouse event (if enabled).
    MouseEvent(crossterm::event::MouseEvent),

    /// Terminal resize event.
    Resize { width: u16, height: u16 },

    // === Navigation Actions ===

    /// Change focused panel.
    FocusPanel(Panel),

    /// Cycle to next panel.
    FocusNext,

    /// Cycle to previous panel.
    FocusPrevious,

    // === File Tree Actions ===

    /// Directory scan completed.
    ScanComplete {
        root: PathBuf,
        nodes: Vec<FileNode>,
        duration: std::time::Duration,
    },

    /// Directory scan failed.
    ScanFailed {
        root: PathBuf,
        error: String,
    },

    /// Lazy load of directory children completed.
    ChildrenLoaded {
        parent: PathBuf,
        children: Vec<FileNode>,
    },

    /// File tree selection changed.
    TreeSelectionChanged {
        path: Option<PathBuf>,
    },

    // === Operation Actions ===

    /// Toggle an operation's enabled state.
    ToggleOperation(usize),

    /// Toggle all operations.
    ToggleAllOperations,

    /// Operation match calculation completed.
    OperationMatchesCalculated {
        op_index: usize,
        matched_files: Vec<PathBuf>,
        stats: OperationStats,
    },

    // === Execution Actions ===

    /// Start execution of enabled operations.
    ExecutionStart {
        total_files: u64,
        operations: Vec<OperationType>,
    },

    /// Progress update during execution.
    ProgressUpdate {
        processed: u64,
        current_file: PathBuf,
        current_operation: String,
    },

    /// Single file operation completed.
    FileOperationComplete {
        file: PathBuf,
        operation: OperationType,
        result: Result<(), String>,
    },

    /// Execution completed successfully.
    ExecutionComplete {
        duration: std::time::Duration,
        counters: ExecutionCounters,
    },

    /// Execution cancelled by user.
    ExecutionCancelled {
        at_file: u64,
        of_total: u64,
    },

    /// Execution failed with error.
    ExecutionFailed {
        error: String,
        at_file: PathBuf,
    },

    /// Pause execution.
    ExecutionPause,

    /// Resume execution.
    ExecutionResume,

    // === Dialog Actions ===

    /// Show confirmation dialog.
    ShowConfirmDialog {
        title: String,
        message: String,
        on_confirm: Box<Action>,
    },

    /// Show move dialog for selected file(s).
    ShowMoveDialog {
        files: Vec<PathBuf>,
    },

    /// Show conflict resolution dialog.
    ShowConflictDialog {
        source: PathBuf,
        target: PathBuf,
    },

    /// Dialog confirmed.
    DialogConfirm,

    /// Dialog cancelled.
    DialogCancel,

    /// Dialog option selected.
    DialogSelect(usize),

    // === Move Actions ===

    /// Quick move to preset target (1-9 keys).
    QuickMove {
        files: Vec<PathBuf>,
        target_key: char,
    },

    /// Move to custom path.
    CustomMove {
        files: Vec<PathBuf>,
        target: PathBuf,
    },

    /// Conflict resolution chosen.
    ResolveConflict {
        resolution: ConflictResolution,
    },

    // === Logging Actions ===

    /// Add log entry.
    Log(LogEntry),

    /// Clear log.
    ClearLog,

    // === Mode Actions ===

    /// Enter search mode.
    EnterSearch,

    /// Exit search mode.
    ExitSearch,

    /// Update search query.
    SearchQueryChanged(String),

    /// Show help panel.
    ShowHelp,

    /// Hide help panel.
    HideHelp,
}

/// Result type for action handlers.
pub enum ActionResult {
    /// Action was handled, continue processing.
    Handled,

    /// Action was not handled, propagate to parent.
    NotHandled,

    /// Action caused an error.
    Error(String),

    /// Action requires application exit.
    Exit,
}
