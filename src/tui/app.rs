//! Application state and core TUI logic

use std::collections::VecDeque;
use std::path::PathBuf;

use tokio::sync::mpsc;

use super::components::{FileTreeComponent, OperationsComponent, PreviewComponent};
use super::state::{Action, ExecutionProgress, LogEntry, LogPersistence};

/// Panel identifiers for the three-panel layout
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    FileTree,
    Operations,
    Preview,
}

impl Panel {
    /// Get the next panel in cycle order
    pub fn next(self) -> Self {
        match self {
            Panel::FileTree => Panel::Operations,
            Panel::Operations => Panel::Preview,
            Panel::Preview => Panel::FileTree,
        }
    }

    /// Get the previous panel in cycle order
    pub fn previous(self) -> Self {
        match self {
            Panel::FileTree => Panel::Preview,
            Panel::Operations => Panel::FileTree,
            Panel::Preview => Panel::Operations,
        }
    }
}

/// Application mode states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Dialog types that can be displayed
#[derive(Debug, Clone)]
pub enum Dialog {
    /// Confirmation dialog (quit, dangerous operations)
    Confirm {
        title: String,
        message: String,
        on_confirm: DialogAction,
    },
    /// Move file dialog
    Move {
        source_files: Vec<PathBuf>,
        selected_target: Option<usize>,
        custom_path: String,
    },
    /// Conflict resolution dialog
    Conflict {
        source: PathBuf,
        target: PathBuf,
        selected_option: usize,
    },
    /// Help panel
    Help { scroll_offset: usize },
}

/// Actions to perform on dialog confirmation
#[derive(Debug, Clone)]
pub enum DialogAction {
    Quit,
    ExecuteOperations,
    MoveFiles(PathBuf),
}

/// Main application state
pub struct App {
    /// Currently focused panel
    pub focused_panel: Panel,

    /// Current application mode
    pub mode: AppMode,

    /// File tree component
    pub file_tree: FileTreeComponent,

    /// Operations list component
    pub operations: OperationsComponent,

    /// Preview panel component
    pub preview: PreviewComponent,

    /// Execution state (when running)
    pub execution: Option<ExecutionProgress>,

    /// Active dialog (if any)
    pub dialog: Option<Dialog>,

    /// Log entries for display
    pub logs: VecDeque<LogEntry>,

    /// Log persistence handler (T085-T087)
    pub log_persistence: LogPersistence,

    /// Action channel sender
    pub action_tx: mpsc::UnboundedSender<Action>,

    /// Source directory path
    pub source_dir: PathBuf,

    /// Whether the app should quit
    pub should_quit: bool,

    /// Search query (when in search mode)
    pub search_query: String,

    /// Flag to trigger file tree refresh (T066)
    pub needs_refresh: bool,
}

impl App {
    /// Create a new App instance
    pub fn new(source_dir: PathBuf, action_tx: mpsc::UnboundedSender<Action>) -> Self {
        let log_persistence = LogPersistence::new();
        // Write session start marker to log file
        log_persistence.write_session_start(&source_dir);

        Self {
            focused_panel: Panel::FileTree,
            mode: AppMode::Normal,
            file_tree: FileTreeComponent::new(source_dir.clone()),
            operations: OperationsComponent::new(),
            preview: PreviewComponent::new(),
            execution: None,
            dialog: None,
            logs: VecDeque::with_capacity(1000),
            log_persistence,
            action_tx,
            source_dir,
            should_quit: false,
            search_query: String::new(),
            needs_refresh: false,
        }
    }

    /// Switch to the next panel
    pub fn next_panel(&mut self) {
        if self.mode == AppMode::Normal && self.dialog.is_none() {
            self.focused_panel = self.focused_panel.next();
        }
    }

    /// Switch to the previous panel
    pub fn previous_panel(&mut self) {
        if self.mode == AppMode::Normal && self.dialog.is_none() {
            self.focused_panel = self.focused_panel.previous();
        }
    }

    /// Show quit confirmation dialog
    pub fn show_quit_dialog(&mut self) {
        self.dialog = Some(Dialog::Confirm {
            title: "Quit".to_string(),
            message: "Are you sure you want to quit?".to_string(),
            on_confirm: DialogAction::Quit,
        });
    }

    /// Show help panel
    pub fn show_help(&mut self) {
        self.dialog = Some(Dialog::Help { scroll_offset: 0 });
        self.mode = AppMode::Help;
    }

    /// Hide dialog/help
    pub fn hide_dialog(&mut self) {
        self.dialog = None;
        if self.mode == AppMode::Help {
            self.mode = AppMode::Normal;
        }
    }

    /// Confirm current dialog action
    pub fn confirm_dialog(&mut self) {
        // Handle Dialog::Confirm
        if let Some(Dialog::Confirm { on_confirm, .. }) = &self.dialog {
            match on_confirm {
                DialogAction::Quit => {
                    self.should_quit = true;
                }
                DialogAction::ExecuteOperations => {
                    self.start_execution();
                }
                DialogAction::MoveFiles(target) => {
                    self.execute_move_files(target.clone());
                }
            }
        }
        // Handle Dialog::Move (T110 fix)
        else if let Some(Dialog::Move {
            source_files,
            selected_target,
            custom_path,
        }) = self.dialog.clone()
        {
            // Determine target path
            let targets = self.get_move_targets();
            let target_path = if !custom_path.is_empty() {
                PathBuf::from(&custom_path)
            } else if let Some(idx) = selected_target {
                if idx < targets.len() {
                    self.source_dir.join(&targets[idx])
                } else {
                    return; // Invalid selection
                }
            } else {
                return; // No target selected
            };

            // Execute move for all source files
            for source in &source_files {
                if let Some(file_name) = source.file_name() {
                    let dest = target_path.join(file_name);
                    // Create target directory if needed
                    if !target_path.exists() {
                        let _ = std::fs::create_dir_all(&target_path);
                    }
                    match std::fs::rename(source, &dest) {
                        Ok(_) => {
                            self.add_log(super::state::LogEntry::success(format!(
                                "Moved: {} -> {}",
                                source.display(),
                                dest.display()
                            )));
                        }
                        Err(e) => {
                            self.add_log(super::state::LogEntry::error(format!(
                                "Failed to move {}: {}",
                                source.display(),
                                e
                            )));
                        }
                    }
                }
            }
            // Clear multi-select after move
            if self.file_tree.is_multi_select() {
                self.file_tree.toggle_multi_select();
            }
            // Trigger file tree refresh (T066)
            self.needs_refresh = true;
        }
        self.hide_dialog();
    }

    /// Execute move files operation
    fn execute_move_files(&mut self, target: PathBuf) {
        let files_to_move =
            if self.file_tree.is_multi_select() && self.file_tree.selected_count() > 0 {
                self.file_tree.selected_files()
            } else if let Some(path) = self.file_tree.selected_path() {
                vec![path]
            } else {
                vec![]
            };

        for source in files_to_move {
            if let Some(file_name) = source.file_name() {
                let dest = target.join(file_name);
                if !target.exists() {
                    let _ = std::fs::create_dir_all(&target);
                }
                match std::fs::rename(&source, &dest) {
                    Ok(_) => {
                        self.add_log(super::state::LogEntry::success(format!(
                            "Moved: {} -> {}",
                            source.display(),
                            dest.display()
                        )));
                    }
                    Err(e) => {
                        self.add_log(super::state::LogEntry::error(format!(
                            "Failed to move {}: {}",
                            source.display(),
                            e
                        )));
                    }
                }
            }
        }
        if self.file_tree.is_multi_select() {
            self.file_tree.toggle_multi_select();
        }
        // Trigger file tree refresh (T066)
        self.needs_refresh = true;
    }

    /// Get predefined move targets (T052)
    pub fn get_move_targets(&self) -> Vec<String> {
        super::state::MoveTarget::default_presets()
            .into_iter()
            .map(|t| t.path)
            .collect()
    }

    /// Get move targets with full metadata (T052)
    pub fn get_move_targets_full(&self) -> Vec<super::state::MoveTarget> {
        super::state::MoveTarget::default_presets()
    }

    /// Start execution mode
    pub fn start_execution(&mut self) {
        // T101: Log execution start
        self.add_log(super::state::LogEntry::info(format!(
            "Starting execution in: {}",
            self.source_dir.display()
        )));

        self.mode = AppMode::Executing;
        // Initialize execution progress
        let enabled_ops = self.operations.enabled_operation_count();
        self.execution = Some(ExecutionProgress::new(enabled_ops as u64));
    }

    /// Complete execution and return to normal mode
    pub fn complete_execution(&mut self) {
        self.mode = AppMode::Normal;
        self.execution = None;
    }

    /// Cancel execution
    pub fn cancel_execution(&mut self) {
        self.mode = AppMode::Normal;
        self.execution = None;
    }

    /// Add a log entry
    pub fn add_log(&mut self, entry: LogEntry) {
        // Persist to file (T085-T087)
        self.log_persistence.write(&entry);

        self.logs.push_back(entry);
        // Keep only last 1000 entries
        while self.logs.len() > 1000 {
            self.logs.pop_front();
        }
    }

    /// Enter search mode
    pub fn enter_search(&mut self) {
        self.mode = AppMode::Search;
        self.search_query.clear();
    }

    /// Exit search mode
    pub fn exit_search(&mut self) {
        self.mode = AppMode::Normal;
        self.search_query.clear();
    }
}
