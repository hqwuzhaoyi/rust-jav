# Data Model: LazyGit 风格 TUI 界面

**Date**: 2025-12-07
**Branch**: `001-lazygit-style-tui`

## Entity Relationship Diagram

```
┌─────────────────┐       ┌─────────────────┐
│    App State    │       │   TreeState     │
├─────────────────┤       ├─────────────────┤
│ focused_panel   │       │ selected        │
│ mode            │       │ offset          │
│ file_tree       │──────▶│ expanded        │
│ operations      │       └─────────────────┘
│ preview         │
│ execution       │       ┌─────────────────┐
│ dialogs         │       │    FileNode     │
│ action_tx       │       ├─────────────────┤
└─────────────────┘       │ path            │
                          │ name            │
┌─────────────────┐       │ is_dir          │
│   Operation     │       │ size            │
├─────────────────┤       │ children[]      │◀──┐
│ op_type         │       │ depth           │   │
│ name            │       │ pending_op      │   │
│ description     │       │ status          │───┘
│ enabled         │       └─────────────────┘
│ matched_files[] │
│ stats           │       ┌─────────────────┐
└─────────────────┘       │   MoveTarget    │
                          ├─────────────────┤
┌─────────────────┐       │ path            │
│ExecutionProgress│       │ shortcut_key    │
├─────────────────┤       │ is_preset       │
│ total_files     │       │ last_used       │
│ processed       │       └─────────────────┘
│ current_op      │
│ current_file    │       ┌─────────────────┐
│ start_time      │       │    LogEntry     │
│ status          │       ├─────────────────┤
└─────────────────┘       │ timestamp       │
                          │ level           │
                          │ message         │
                          │ file_path       │
                          └─────────────────┘
```

## Core Entities

### 1. FileNode

Represents a file system entry in the directory tree.

```rust
#[derive(Clone, Debug)]
pub struct FileNode {
    /// Absolute path to the file/directory
    pub path: PathBuf,

    /// Display name (filename only)
    pub name: String,

    /// Whether this is a directory
    pub is_dir: bool,

    /// File size in bytes (0 for directories)
    pub size: u64,

    /// Child nodes (empty if not loaded or not a directory)
    pub children: Vec<FileNode>,

    /// Nesting depth (0 = root)
    pub depth: usize,

    /// Pending operation to be applied
    pub pending_operation: Option<OperationType>,

    /// Current status in the workflow
    pub status: FileStatus,

    /// Whether children have been loaded
    pub children_loaded: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FileStatus {
    /// No operation pending
    Unchanged,
    /// Marked for move
    ToMove(PathBuf),
    /// Marked for deletion
    ToDelete,
    /// Marked for rename
    ToRename(String),
    /// Selected in multi-select mode
    Selected,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OperationType {
    DeleteEmptyFolders,
    MoveChineseSubtitle,
    MoveUncensored,
    RenameUppercase,
    RemovePrefix,
    RemoveAds,
    ManualMove(PathBuf),
}
```

**Validation Rules**:
- `path` must be absolute and exist on filesystem
- `name` extracted from path, cannot be empty
- `children` only populated for directories when `children_loaded = true`
- `depth` increments by 1 for each nesting level

**State Transitions**:
```
Unchanged ──[match pattern]──▶ ToMove/ToDelete/ToRename
Unchanged ──[manual select]──▶ Selected
Selected ──[press m]──▶ ToMove(target)
ToMove ──[execute]──▶ Unchanged (file moved)
ToDelete ──[execute]──▶ (node removed)
```

### 2. TreeState

UI state for the file tree panel (separate from data).

```rust
#[derive(Default)]
pub struct TreeState {
    /// Currently selected index in flat view
    pub selected: Option<usize>,

    /// Scroll offset for viewport
    pub offset: usize,

    /// Set of expanded directory paths
    pub expanded: HashSet<PathBuf>,

    /// Multi-select mode active
    pub multi_select_mode: bool,

    /// Selected items in multi-select mode
    pub multi_selected: HashSet<PathBuf>,
}
```

### 3. Operation

Represents a batch operation that can be enabled/disabled.

```rust
#[derive(Clone, Debug)]
pub struct Operation {
    /// Operation type identifier
    pub op_type: OperationType,

    /// Display name
    pub name: String,

    /// Description shown in preview
    pub description: String,

    /// Whether operation is enabled
    pub enabled: bool,

    /// Files matched by this operation
    pub matched_files: Vec<PathBuf>,

    /// Statistics about impact
    pub stats: OperationStats,
}

#[derive(Clone, Debug, Default)]
pub struct OperationStats {
    /// Number of files affected
    pub file_count: usize,

    /// Total size of affected files
    pub total_size: u64,

    /// Space to be freed (for delete operations)
    pub space_freed: u64,
}
```

**Predefined Operations** (from `config.rs`):
1. Delete empty folders
2. Move Chinese subtitle videos (-C, -ch suffix)
3. Move uncensored videos
4. Rename to uppercase
5. Remove prefixes (from PREFIXES array)
6. Remove ad suffixes

### 4. MoveTarget

Represents a destination for manual move operations.

```rust
#[derive(Clone, Debug)]
pub struct MoveTarget {
    /// Target directory path
    pub path: PathBuf,

    /// Keyboard shortcut (1-9, or None for custom)
    pub shortcut_key: Option<char>,

    /// Whether this is a preset target
    pub is_preset: bool,

    /// Last time this target was used
    pub last_used: Option<SystemTime>,
}
```

**Preset Targets** (keys 1-9):
| Key | Path | Description |
|-----|------|-------------|
| 1 | `CHINESE/` | Chinese subtitle videos |
| 2 | `UNCENSORED/` | Uncensored videos |
| 3 | `SUBTITLED/` | General subtitled |
| 4 | `4K/` | 4K content |
| 5 | `VR/` | VR content |
| 6-9 | (user-defined) | Custom targets |

### 5. ExecutionProgress

Tracks progress during batch execution.

```rust
#[derive(Clone, Debug)]
pub struct ExecutionProgress {
    /// Total number of files to process
    pub total_files: u64,

    /// Number of files processed
    pub processed_files: u64,

    /// Current operation name
    pub current_operation: String,

    /// Current file being processed
    pub current_file: PathBuf,

    /// When execution started
    pub start_time: Instant,

    /// Execution status
    pub status: ExecutionStatus,

    /// Detailed counters
    pub counters: ExecutionCounters,
}

#[derive(Clone, Debug, Default)]
pub struct ExecutionCounters {
    pub moved: usize,
    pub deleted: usize,
    pub renamed: usize,
    pub skipped: usize,
    pub errors: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExecutionStatus {
    Idle,
    Running,
    Paused,
    Completed,
    Cancelled,
    Failed(String),
}
```

### 6. LogEntry

Represents a log message displayed in the TUI.

```rust
#[derive(Clone, Debug)]
pub struct LogEntry {
    /// When the event occurred
    pub timestamp: DateTime<Local>,

    /// Log level
    pub level: LogLevel,

    /// Log message
    pub message: String,

    /// Associated file path (if any)
    pub file_path: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
}
```

## Application State

### App (Root State)

```rust
pub struct App {
    /// Currently focused panel
    pub focused_panel: Panel,

    /// Application mode
    pub mode: AppMode,

    /// File tree component state
    pub file_tree: FileTreeComponent,

    /// Operations list component state
    pub operations: OperationsComponent,

    /// Preview panel component state
    pub preview: PreviewComponent,

    /// Execution state (when running)
    pub execution: Option<ExecutionProgress>,

    /// Active dialog (if any)
    pub dialog: Option<Dialog>,

    /// Log entries for display
    pub logs: VecDeque<LogEntry>,

    /// Action channel sender (for async updates)
    pub action_tx: mpsc::UnboundedSender<Action>,

    /// Source directory path
    pub source_dir: PathBuf,

    /// Whether there are unsaved changes
    pub has_changes: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Panel {
    FileTree,
    Operations,
    Preview,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppMode {
    /// Normal browsing mode
    Normal,
    /// Executing operations
    Executing,
    /// Search mode (/ key)
    Search,
    /// Help panel visible
    Help,
}

#[derive(Clone, Debug)]
pub enum Dialog {
    Confirm {
        title: String,
        message: String,
        on_confirm: DialogAction,
    },
    Move {
        source_files: Vec<PathBuf>,
        targets: Vec<MoveTarget>,
        selected_target: Option<usize>,
        custom_path: String,
    },
    Conflict {
        source: PathBuf,
        target: PathBuf,
        options: Vec<ConflictResolution>,
    },
}

#[derive(Clone, Debug)]
pub enum ConflictResolution {
    Skip,
    Overwrite,
    Rename(String),
}
```

## Action Types

```rust
pub enum Action {
    // Tick events
    Tick,

    // User input
    KeyPress(KeyEvent),

    // File operations
    ScanComplete(Vec<FileNode>),
    FileOperationComplete {
        file: PathBuf,
        result: Result<(), String>,
    },

    // Progress updates
    ProgressUpdate {
        processed: u64,
        current_file: PathBuf,
        current_operation: String,
    },

    // Execution lifecycle
    ExecutionStart { total_files: u64 },
    ExecutionComplete,
    ExecutionCancelled,

    // Dialog responses
    DialogConfirm,
    DialogCancel,

    // Navigation
    FocusPanel(Panel),
    SelectItem(usize),

    // Operations
    ToggleOperation(usize),
    ToggleAllOperations,

    // Log
    Log(LogEntry),
}
```

## Data Flow

```
User Input ──▶ Event Handler ──▶ Action ──▶ State Update ──▶ Render

Async Operation ──▶ Action Channel ──▶ State Update ──▶ Render
```

1. **Input Phase**: Keyboard events captured by crossterm EventStream
2. **Dispatch Phase**: Events converted to Actions based on current focus/mode
3. **Update Phase**: App state modified based on Action
4. **Render Phase**: UI re-rendered from updated state (immediate mode)
