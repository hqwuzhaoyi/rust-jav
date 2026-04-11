//! UI rendering module
//!
//! Contains the main rendering logic for the three-panel TUI layout.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

use super::app::{App, AppMode, Panel};
use super::components::{render_dialog, Component};

/// Draw the complete UI
pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();

    // Main layout: status bar, content area, log panel, help bar
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Status bar
            Constraint::Min(10),   // Main content (three panels)
            Constraint::Length(8), // Log panel
            Constraint::Length(1), // Help bar
        ])
        .split(size);

    // Draw status bar
    draw_status_bar(f, app, main_chunks[0]);

    // Draw main content area (three panels)
    draw_panels(f, app, main_chunks[1]);

    // Draw log panel
    draw_log_panel(f, app, main_chunks[2]);

    // Draw help bar
    draw_help_bar(f, app, main_chunks[3]);

    // Draw progress overlay when executing (T078)
    if app.mode == AppMode::Executing {
        draw_progress_overlay(f, app, size);
    }

    // Draw dialog overlay if any
    if let Some(ref dialog) = app.dialog {
        render_dialog(f, dialog);
    }
}

/// Draw the status bar at the top
fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let mode_str = match app.mode {
        AppMode::Normal => "NORMAL",
        AppMode::Executing => "EXECUTING",
        AppMode::Search => "SEARCH",
        AppMode::Help => "HELP",
    };

    let mode_style = match app.mode {
        AppMode::Normal => Style::default().fg(Color::Green),
        AppMode::Executing => Style::default().fg(Color::Yellow),
        AppMode::Search => Style::default().fg(Color::Cyan),
        AppMode::Help => Style::default().fg(Color::Blue),
    };

    let status = Line::from(vec![
        Span::styled(
            " rust-jav ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("│ "),
        Span::styled(mode_str, mode_style.add_modifier(Modifier::BOLD)),
        Span::raw(" │ "),
        Span::styled(
            app.source_dir.display().to_string(),
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let status_bar =
        Paragraph::new(status).style(Style::default().bg(Color::DarkGray).fg(Color::White));

    f.render_widget(status_bar, area);
}

/// Draw the three-panel content area
fn draw_panels(f: &mut Frame, app: &App, area: Rect) {
    // Three-panel layout: 30% file tree, 40% operations, 30% preview
    let panel_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // File tree
            Constraint::Percentage(40), // Operations
            Constraint::Percentage(30), // Preview
        ])
        .split(area);

    // Draw file tree panel
    app.file_tree.render(
        f,
        panel_chunks[0],
        app.focused_panel == Panel::FileTree && app.dialog.is_none(),
    );

    // Draw operations panel
    app.operations.render(
        f,
        panel_chunks[1],
        app.focused_panel == Panel::Operations && app.dialog.is_none(),
    );

    // Draw preview panel
    app.preview.render(
        f,
        panel_chunks[2],
        app.focused_panel == Panel::Preview && app.dialog.is_none(),
    );
}

/// Draw the log panel
fn draw_log_panel(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Logs ");

    // Build log lines from app.logs
    let log_lines: Vec<Line> = app
        .logs
        .iter()
        .rev()
        .take(area.height as usize - 2)
        .map(|entry| {
            let level_style = match entry.level {
                super::state::LogLevel::Info => Style::default().fg(Color::Blue),
                super::state::LogLevel::Success => Style::default().fg(Color::Green),
                super::state::LogLevel::Warning => Style::default().fg(Color::Yellow),
                super::state::LogLevel::Error => Style::default().fg(Color::Red),
            };

            let level_str = match entry.level {
                super::state::LogLevel::Info => "INFO",
                super::state::LogLevel::Success => "OK",
                super::state::LogLevel::Warning => "WARN",
                super::state::LogLevel::Error => "ERR",
            };

            let time_str = entry.timestamp.format("%H:%M:%S").to_string();

            Line::from(vec![
                Span::styled(
                    format!("[{}] ", time_str),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(format!("{:4} ", level_str), level_style),
                Span::raw(&entry.message),
            ])
        })
        .collect();

    // If no logs, show placeholder
    let content = if log_lines.is_empty() {
        vec![Line::from(Span::styled(
            "No logs yet. Operations will be logged here.",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        log_lines
    };

    let paragraph = Paragraph::new(content).block(block);

    f.render_widget(paragraph, area);
}

/// Draw the help bar at the bottom
fn draw_help_bar(f: &mut Frame, app: &App, area: Rect) {
    let shortcuts = match app.mode {
        AppMode::Normal => get_normal_mode_shortcuts(app),
        AppMode::Executing => vec![("Ctrl+C", "Cancel"), ("j/k", "Scroll log")],
        AppMode::Search => vec![("Enter", "Apply"), ("Esc", "Cancel")],
        AppMode::Help => vec![("j/k", "Scroll"), ("Esc/F1", "Close")],
    };

    let mut spans = Vec::new();
    for (i, (key, desc)) in shortcuts.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  │  "));
        }
        spans.push(Span::styled(
            format!("[{}]", key),
            Style::default().fg(Color::Yellow),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(*desc, Style::default().fg(Color::DarkGray)));
    }

    // Add search query display in search mode
    if app.mode == AppMode::Search {
        spans.push(Span::raw("  │  "));
        spans.push(Span::styled("Search: ", Style::default().fg(Color::Cyan)));
        spans.push(Span::styled(
            &app.search_query,
            Style::default().fg(Color::White),
        ));
        spans.push(Span::styled("█", Style::default().fg(Color::White))); // Cursor
    }

    let help_line = Line::from(spans);
    let help_bar = Paragraph::new(help_line).style(Style::default().bg(Color::Black));

    f.render_widget(help_bar, area);
}

/// Get shortcuts for normal mode based on focused panel
fn get_normal_mode_shortcuts(app: &App) -> Vec<(&'static str, &'static str)> {
    let common = vec![("Tab", "Next panel"), ("q", "Quit"), ("F1", "Help")];

    let panel_specific: Vec<(&'static str, &'static str)> = match app.focused_panel {
        Panel::FileTree => vec![
            ("j/k", "Navigate"),
            ("l/h", "Expand/Collapse"),
            ("a", "Select all"),
            ("m", "Move"),
            ("v", "Multi-select"),
        ],
        Panel::Operations => vec![
            ("j/k", "Navigate"),
            ("Space", "Toggle"),
            ("a", "Toggle all"),
            ("Enter", "Execute"),
        ],
        Panel::Preview => vec![("j/k", "Scroll")],
    };

    let mut shortcuts = panel_specific;
    shortcuts.extend(common);
    shortcuts
}

/// Draw an execution progress overlay (when in executing mode)
pub fn draw_progress_overlay(f: &mut Frame, app: &App, area: Rect) {
    if app.mode != AppMode::Executing {
        return;
    }

    if let Some(ref progress) = app.execution {
        // Create a centered progress dialog
        let progress_area = centered_rect(70, 50, area);

        // Clear the area first
        f.render_widget(ratatui::widgets::Clear, progress_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(" Executing Operations ");

        let inner = block.inner(progress_area);
        f.render_widget(block, progress_area);

        // Layout for progress content
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Current operation
                Constraint::Length(3), // Progress bar
                Constraint::Length(3), // Statistics
                Constraint::Length(2), // ETA
                Constraint::Min(1),    // Spacer
                Constraint::Length(1), // Help hint
            ])
            .split(inner);

        // Current operation
        let current_op = progress
            .current_operation
            .as_deref()
            .unwrap_or("Preparing...");
        let current_file = progress
            .current_file
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let op_text = if current_file.is_empty() {
            format!("Operation: {}", current_op)
        } else {
            format!("Operation: {} - {}", current_op, current_file)
        };

        let op_paragraph = Paragraph::new(op_text)
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::NONE));
        f.render_widget(op_paragraph, chunks[0]);

        // Progress bar
        let percent = if progress.total_files > 0 {
            ((progress.processed_files as f64 / progress.total_files as f64) * 100.0) as u16
        } else {
            0
        };

        let progress_label = format!(
            "{}/{} operations ({}%)",
            progress.processed_files, progress.total_files, percent
        );

        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(" Progress "))
            .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
            .percent(percent)
            .label(progress_label);
        f.render_widget(gauge, chunks[1]);

        // Statistics (T080)
        let stats_text = format!(
            "Success: {}  |  Errors: {}  |  Skipped: {}",
            progress.success_count, progress.error_count, progress.skip_count
        );

        let stats_style = if progress.error_count > 0 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Green)
        };

        let stats_paragraph = Paragraph::new(stats_text)
            .style(stats_style)
            .alignment(ratatui::layout::Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(" Statistics "));
        f.render_widget(stats_paragraph, chunks[2]);

        // ETA
        let elapsed = progress.start_time.elapsed();
        let eta_text =
            if progress.processed_files > 0 && progress.total_files > progress.processed_files {
                let rate = progress.processed_files as f64 / elapsed.as_secs_f64();
                let remaining = (progress.total_files - progress.processed_files) as f64 / rate;
                format!(
                    "Elapsed: {:.1}s  |  ETA: {:.1}s  |  Rate: {:.1} ops/s",
                    elapsed.as_secs_f64(),
                    remaining,
                    rate
                )
            } else {
                format!("Elapsed: {:.1}s", elapsed.as_secs_f64())
            };

        let eta_paragraph = Paragraph::new(eta_text)
            .style(Style::default().fg(Color::Cyan))
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(eta_paragraph, chunks[3]);

        // Help hint
        let help_text = "Press Esc or Ctrl+C to cancel";
        let help_paragraph = Paragraph::new(help_text)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(help_paragraph, chunks[5]);
    }
}

/// Create a centered rect of given percentage size
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
