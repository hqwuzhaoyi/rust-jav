//! Event handling and main event loop

use std::io::{self, Stdout};
use std::time::Duration;

use color_eyre::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use super::app::{App, AppMode, Dialog};
use super::ui::draw;

/// Terminal type alias
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal for TUI mode
pub fn init_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore the terminal to normal mode
pub fn restore_terminal(terminal: &mut Tui) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Minimum terminal size for proper TUI display
const MIN_TERMINAL_WIDTH: u16 = 80;
const MIN_TERMINAL_HEIGHT: u16 = 24;

/// Run the main application event loop
pub async fn run_app(terminal: &mut Tui, mut app: App) -> Result<()> {
    // T097: Check terminal size
    let size = terminal.size()?;
    if size.width < MIN_TERMINAL_WIDTH || size.height < MIN_TERMINAL_HEIGHT {
        app.add_log(super::state::LogEntry::warning(format!(
            "Terminal size {}x{} is smaller than recommended {}x{}",
            size.width, size.height, MIN_TERMINAL_WIDTH, MIN_TERMINAL_HEIGHT
        )));
    }

    // Log startup
    app.add_log(super::state::LogEntry::info("rust-jav TUI started"));
    app.add_log(super::state::LogEntry::info(format!(
        "Scanning directory: {}",
        app.source_dir.display()
    )));

    // Initial directory scan
    app.file_tree.scan_directory().await;

    // Log scan complete with file count
    let file_count = app.file_tree.node_count();
    app.add_log(super::state::LogEntry::success(format!(
        "Directory scan complete: {} items found",
        file_count
    )));

    // T099: Warn about large directories
    if file_count > 10000 {
        app.add_log(super::state::LogEntry::warning(format!(
            "Large directory detected ({} items). Performance may be affected.",
            file_count
        )));
    }

    // Analyze operations to find affected files
    app.add_log(super::state::LogEntry::info("Analyzing operations..."));
    let analysis = crate::application::ApplicationServices::new()
        .operations()
        .analyze(app.source_dir.clone())
        .await;
    app.operations.update_all_affected(analysis);
    app.add_log(super::state::LogEntry::success(
        "Operation analysis complete",
    ));

    // Initial preview update based on focused panel
    update_preview_for_panel(&mut app);

    loop {
        // Draw the UI
        terminal.draw(|f| draw(f, &app))?;

        // Handle execution mode - run operations
        if app.mode == AppMode::Executing {
            run_execution_step(&mut app).await;
        }

        // Poll for events with timeout
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events (not release)
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Handle events based on current mode and dialog state
                if app.dialog.is_some() {
                    handle_dialog_event(&mut app, key.code, key.modifiers);
                } else {
                    match app.mode {
                        AppMode::Normal => handle_normal_event(&mut app, key.code, key.modifiers),
                        AppMode::Executing => {
                            handle_executing_event(&mut app, key.code, key.modifiers)
                        }
                        AppMode::Search => handle_search_event(&mut app, key.code),
                        AppMode::Help => handle_help_event(&mut app, key.code),
                    }
                }
            }
        }

        // T066: Check if file tree needs refresh after move operations
        if app.needs_refresh {
            app.needs_refresh = false;
            app.add_log(super::state::LogEntry::info("Refreshing file list..."));
            app.file_tree.scan_directory().await;
            // Re-analyze operations after refresh
            let analysis = crate::application::ApplicationServices::new()
                .operations()
                .analyze(app.source_dir.clone())
                .await;
            app.operations.update_all_affected(analysis);
            app.add_log(super::state::LogEntry::success("File list refreshed"));
            // Update preview
            update_preview_for_panel(&mut app);
        }

        // Check if we should quit
        if app.should_quit {
            break;
        }
    }

    Ok(())
}

/// Handle events when a dialog is open
fn handle_dialog_event(app: &mut App, key: KeyCode, _modifiers: KeyModifiers) {
    match &mut app.dialog {
        Some(Dialog::Confirm { .. }) => match key {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                app.confirm_dialog();
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                app.hide_dialog();
            }
            _ => {}
        },
        Some(Dialog::Help { scroll_offset }) => match key {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::F(1) => {
                app.hide_dialog();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                *scroll_offset = scroll_offset.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                *scroll_offset = scroll_offset.saturating_sub(1);
            }
            _ => {}
        },
        Some(Dialog::Move {
            selected_target,
            custom_path,
            ..
        }) => match key {
            KeyCode::Esc => {
                app.hide_dialog();
            }
            KeyCode::Enter => {
                // Confirm move
                app.confirm_dialog();
            }
            KeyCode::Tab => {
                // T057: Path autocomplete
                let completions =
                    super::state::PathCompleter::complete(&app.source_dir, custom_path);
                if let Some(completion) =
                    super::state::PathCompleter::next_completion(&completions, custom_path)
                {
                    *custom_path = completion;
                    *selected_target = None; // Clear preset selection when using custom path
                }
            }
            KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                // Quick target selection (1-9)
                let idx = (c as usize) - ('1' as usize);
                *selected_target = Some(idx);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(idx) = selected_target {
                    *selected_target = Some(idx.saturating_sub(1));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(idx) = selected_target {
                    *selected_target = Some(idx.saturating_add(1));
                } else {
                    *selected_target = Some(0);
                }
            }
            KeyCode::Backspace => {
                custom_path.pop();
            }
            KeyCode::Char(c) => {
                custom_path.push(c);
            }
            _ => {}
        },
        Some(Dialog::Conflict {
            selected_option, ..
        }) => match key {
            KeyCode::Esc => {
                app.hide_dialog();
            }
            KeyCode::Enter => {
                app.confirm_dialog();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                *selected_option = selected_option.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                *selected_option = (*selected_option + 1).min(2);
            }
            KeyCode::Char('1') => *selected_option = 0,
            KeyCode::Char('2') => *selected_option = 1,
            KeyCode::Char('3') => *selected_option = 2,
            _ => {}
        },
        None => {}
    }
}

/// Handle events in normal mode
fn handle_normal_event(app: &mut App, key: KeyCode, modifiers: KeyModifiers) {
    match key {
        // Quit
        KeyCode::Char('q') => {
            app.show_quit_dialog();
        }
        // Help (F1 or ? key) - T112 fix: add ? as alternative to avoid h key conflict
        KeyCode::F(1) | KeyCode::Char('?') => {
            app.show_help();
        }
        // Panel navigation
        KeyCode::Tab => {
            if modifiers.contains(KeyModifiers::SHIFT) {
                app.previous_panel();
            } else {
                app.next_panel();
            }
            // Update preview based on newly focused panel
            update_preview_for_panel(app);
        }
        KeyCode::BackTab => {
            app.previous_panel();
            // Update preview based on newly focused panel
            update_preview_for_panel(app);
        }
        // Search
        KeyCode::Char('/') => {
            app.enter_search();
        }
        // Execute operations
        KeyCode::Enter => {
            if app.operations.has_enabled_operations() {
                app.dialog = Some(Dialog::Confirm {
                    title: "Execute Operations".to_string(),
                    message: "Execute all enabled operations?".to_string(),
                    on_confirm: super::app::DialogAction::ExecuteOperations,
                });
            }
        }
        // Panel-specific keys
        _ => {
            match app.focused_panel {
                super::app::Panel::FileTree => {
                    handle_file_tree_event(app, key, modifiers);
                }
                super::app::Panel::Operations => {
                    handle_operations_event(app, key);
                }
                super::app::Panel::Preview => {
                    // Preview panel is read-only, navigate with j/k
                    match key {
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.preview.scroll_down();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.preview.scroll_up();
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Handle file tree panel events
fn handle_file_tree_event(app: &mut App, key: KeyCode, _modifiers: KeyModifiers) {
    match key {
        KeyCode::Down | KeyCode::Char('j') => {
            app.file_tree.next();
            update_preview_from_file_tree(app);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.file_tree.previous();
            update_preview_from_file_tree(app);
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
            app.file_tree.expand_or_enter();
            update_preview_from_file_tree(app);
        }
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
            app.file_tree.collapse_or_back();
            update_preview_from_file_tree(app);
        }
        KeyCode::Char('m') => {
            // Open move dialog
            if let Some(selected) = app.file_tree.selected_path() {
                app.dialog = Some(Dialog::Move {
                    source_files: vec![selected],
                    selected_target: None,
                    custom_path: String::new(),
                });
            }
        }
        KeyCode::Char('v') => {
            // Toggle multi-select mode
            app.file_tree.toggle_multi_select();
            update_preview_from_file_tree(app);
        }
        KeyCode::Char(' ') if app.file_tree.is_multi_select() => {
            // In multi-select mode, toggle selection
            app.file_tree.toggle_current_selection();
            update_preview_from_file_tree(app);
        }
        KeyCode::Char('a') => {
            // Select/deselect all files (auto-enable multi-select mode)
            if !app.file_tree.is_multi_select() {
                app.file_tree.toggle_multi_select();
            }
            app.file_tree.toggle_select_all();
            update_preview_from_file_tree(app);
        }
        _ => {}
    }
}

/// Handle operations panel events
fn handle_operations_event(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Down | KeyCode::Char('j') => {
            app.operations.next();
            update_preview_from_operation(app);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.operations.previous();
            update_preview_from_operation(app);
        }
        KeyCode::Char(' ') => {
            app.operations.toggle_current();
        }
        KeyCode::Char('a') => {
            app.operations.toggle_all();
        }
        _ => {}
    }
}

/// Handle events in executing mode
fn handle_executing_event(app: &mut App, key: KeyCode, modifiers: KeyModifiers) {
    match key {
        // Ctrl+C or Esc to interrupt
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.add_log(super::state::LogEntry::warning(
                "Execution cancelled by user",
            ));
            app.cancel_execution();
        }
        KeyCode::Esc => {
            app.add_log(super::state::LogEntry::warning(
                "Execution cancelled by user",
            ));
            app.cancel_execution();
        }
        // Scroll preview
        KeyCode::Down | KeyCode::Char('j') => {
            app.preview.scroll_down();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.preview.scroll_up();
        }
        _ => {}
    }
}

/// Run one step of the execution process
async fn run_execution_step(app: &mut App) {
    // Check if execution is still active and get status
    let (is_complete, processed, total) = {
        match &app.execution {
            Some(exec) => (exec.is_complete(), exec.processed_files, exec.total_files),
            None => {
                app.complete_execution();
                return;
            }
        }
    };

    // Check if we're done
    if is_complete {
        app.add_log(super::state::LogEntry::success(format!(
            "Execution complete: {} operations processed",
            processed
        )));

        // Refresh file list after execution
        app.add_log(super::state::LogEntry::info("Refreshing file list..."));
        app.file_tree.scan_directory().await;

        // Re-analyze operations
        let analysis = crate::application::ApplicationServices::new()
            .operations()
            .analyze(app.source_dir.clone())
            .await;
        app.operations.update_all_affected(analysis);

        app.add_log(super::state::LogEntry::success("File list refreshed"));
        app.complete_execution();
        return;
    }

    // Get the current operation to execute
    let enabled_ops: Vec<_> = app
        .operations
        .operations()
        .iter()
        .filter(|op| op.enabled)
        .map(|op| op.op_type)
        .collect();

    if processed as usize >= enabled_ops.len() {
        // Edge case: all operations processed but is_complete wasn't set
        // Refresh file list after execution
        app.add_log(super::state::LogEntry::info("Refreshing file list..."));
        app.file_tree.scan_directory().await;

        // Re-analyze operations
        let analysis = crate::application::ApplicationServices::new()
            .operations()
            .analyze(app.source_dir.clone())
            .await;
        app.operations.update_all_affected(analysis);

        app.add_log(super::state::LogEntry::success("File list refreshed"));
        app.complete_execution();
        return;
    }

    let op_type = enabled_ops[processed as usize];
    let op_name = op_type.name();

    app.add_log(super::state::LogEntry::info(format!(
        "Processing: {} ({}/{})",
        op_name,
        processed + 1,
        total
    )));

    let result = crate::application::ApplicationServices::new()
        .operations()
        .execute(app.source_dir.clone(), op_type)
        .await;

    // Log the result
    if result.success {
        app.add_log(super::state::LogEntry::success(format!(
            "Completed: {} ({} files affected)",
            op_name, result.affected_count
        )));
    } else {
        let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
        app.add_log(super::state::LogEntry::error(format!(
            "Failed: {} - {}",
            op_name, error_msg
        )));
    }

    // T100: Log failed files with permission errors
    for (failed_path, error) in &result.failed_files {
        let file_name = failed_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        app.add_log(super::state::LogEntry::warning(format!(
            "  Failed: {} - {}",
            file_name, error
        )));
    }

    // Update progress
    if let Some(exec) = app.execution.as_mut() {
        exec.processed_files += 1;
    }

    // Small delay to show progress
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

/// Handle events in search mode
fn handle_search_event(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc => {
            app.exit_search();
        }
        KeyCode::Enter => {
            // Apply search and return to normal
            app.file_tree.apply_search(&app.search_query);
            app.exit_search();
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            app.file_tree.filter_by_query(&app.search_query);
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            app.file_tree.filter_by_query(&app.search_query);
        }
        _ => {}
    }
}

/// Handle events in help mode
fn handle_help_event(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::F(1) => {
            app.hide_dialog();
        }
        _ => {}
    }
}

/// Update preview panel - always show selected files, optionally with operation details
fn update_preview(app: &mut App) {
    // Always update selected files
    if app.file_tree.is_multi_select() && app.file_tree.selected_count() > 0 {
        let files = app.file_tree.selected_files();
        app.preview.set_selected_files_preview(files);
    } else if let Some(node) = app.file_tree.selected_node() {
        // Show single file if not in multi-select mode
        app.preview.set_file_preview(node.clone());
    }

    // Update highlighted operation based on current selection in Operations panel
    if let Some(op) = app.operations.selected_operation() {
        app.preview.set_highlighted_operation(Some(op.clone()));
    } else {
        app.preview.set_highlighted_operation(None);
    }
}

/// Update preview from file tree changes (wrapper for compatibility)
fn update_preview_from_file_tree(app: &mut App) {
    update_preview(app);
}

/// Update preview from operation changes (wrapper for compatibility)
fn update_preview_from_operation(app: &mut App) {
    update_preview(app);
}

/// Update preview for panel switch
fn update_preview_for_panel(app: &mut App) {
    update_preview(app);
}
