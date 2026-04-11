//! Dialog components
//!
//! Modal dialogs for confirmations, file moves, and conflicts.

use std::path::Path;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::Dialog;

/// Render a dialog on top of the current UI
pub fn render_dialog(f: &mut Frame, dialog: &Dialog) {
    match dialog {
        Dialog::Confirm { title, message, .. } => {
            render_confirm_dialog(f, title, message);
        }
        Dialog::Move {
            source_files,
            selected_target,
            custom_path,
        } => {
            render_move_dialog(f, source_files, *selected_target, custom_path);
        }
        Dialog::Conflict {
            source,
            target,
            selected_option,
        } => {
            render_conflict_dialog(f, source, target, *selected_option);
        }
        Dialog::Help { scroll_offset } => {
            render_help_dialog(f, *scroll_offset);
        }
    }
}

/// Render a confirmation dialog (centered modal)
fn render_confirm_dialog(f: &mut Frame, title: &str, message: &str) {
    let area = centered_rect(50, 30, f.area());

    // Clear the area behind the dialog
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(" {} ", title));

    let text = Text::from(vec![
        Line::from(""),
        Line::from(Span::styled(message, Style::default())),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "[Y]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Yes   "),
            Span::styled(
                "[N]",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" No"),
        ]),
    ]);

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

/// Render the move file dialog
fn render_move_dialog(
    f: &mut Frame,
    source_files: &[std::path::PathBuf],
    selected_target: Option<usize>,
    custom_path: &str,
) {
    let area = centered_rect(60, 50, f.area());

    // Clear the area behind the dialog
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Move Files ");

    // Build content
    let file_count = source_files.len();
    let file_str = if file_count == 1 {
        source_files[0]
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "1 file".to_string())
    } else {
        format!("{} files", file_count)
    };

    // Quick target directories (examples)
    let quick_targets = ["organized/", "sorted/", "archive/", "temp/", "other/"];

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Moving: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                file_str,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Quick targets:",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    for (i, target) in quick_targets.iter().enumerate() {
        let selected = selected_target == Some(i);
        let marker = if selected { "▶ " } else { "  " };
        let style = if selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}[{}] ", marker, i + 1),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(target.to_string(), style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Custom path: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            if custom_path.is_empty() {
                "(type to enter path)"
            } else {
                custom_path
            },
            if custom_path.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            },
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("[Enter]", Style::default().fg(Color::Green)),
        Span::raw(" Confirm   "),
        Span::styled("[Esc]", Style::default().fg(Color::Red)),
        Span::raw(" Cancel"),
    ]));

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

/// Render the conflict resolution dialog
fn render_conflict_dialog(f: &mut Frame, source: &Path, target: &Path, selected_option: usize) {
    let area = centered_rect(60, 40, f.area());

    // Clear the area behind the dialog
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(" File Conflict ");

    let source_name = source
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| source.display().to_string());

    let target_name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| target.display().to_string());

    let options = [
        ("Skip", "Don't move this file"),
        ("Overwrite", "Replace the existing file"),
        ("Rename", "Add suffix to filename"),
    ];

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Source: ", Style::default().fg(Color::DarkGray)),
            Span::styled(source_name, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("Target exists: ", Style::default().fg(Color::DarkGray)),
            Span::styled(target_name, Style::default().fg(Color::Red)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Choose an action:",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
    ];

    for (i, (name, desc)) in options.iter().enumerate() {
        let selected = i == selected_option;
        let marker = if selected { "▶ " } else { "  " };
        let style = if selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}[{}] ", marker, i + 1),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(name.to_string(), style),
            Span::styled(format!(" - {}", desc), Style::default().fg(Color::DarkGray)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("[Enter]", Style::default().fg(Color::Green)),
        Span::raw(" Confirm   "),
        Span::styled("[Esc]", Style::default().fg(Color::Red)),
        Span::raw(" Cancel"),
    ]));

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

/// Render the help dialog
fn render_help_dialog(f: &mut Frame, _scroll_offset: usize) {
    let area = centered_rect(70, 80, f.area());

    // Clear the area behind the dialog
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .title(" Help (F1 or ? to close) ");

    let help_sections = vec![
        (
            "Navigation",
            vec![
                ("j / ↓", "Move down"),
                ("k / ↑", "Move up"),
                ("l / → / Enter", "Expand / Enter"),
                ("h / ← / Backspace", "Collapse / Back"),
                ("Tab", "Next panel"),
                ("Shift+Tab", "Previous panel"),
            ],
        ),
        (
            "File Operations",
            vec![
                ("m", "Move selected file(s)"),
                ("v", "Toggle multi-select mode"),
                ("Space", "Toggle selection (multi-select)"),
                ("a", "Select/deselect all files"),
                ("/", "Search files"),
            ],
        ),
        (
            "Operations Panel",
            vec![
                ("Space", "Toggle operation"),
                ("a", "Toggle all operations"),
                ("Enter", "Execute enabled operations"),
            ],
        ),
        (
            "Execution Mode",
            vec![
                ("Ctrl+C / Esc", "Cancel execution"),
                ("", "Progress shows in overlay"),
                ("", "Logs update in real-time"),
                ("", "Stats: success/error/skip"),
            ],
        ),
        (
            "General",
            vec![
                ("q", "Quit (with confirmation)"),
                ("F1 / ?", "Show/hide help"),
                ("Esc", "Close dialog / Cancel"),
            ],
        ),
    ];

    let mut items: Vec<ListItem> = Vec::new();

    for (section, keys) in help_sections {
        items.push(ListItem::new(Line::from(vec![Span::styled(
            section.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )])));
        items.push(ListItem::new(Line::from("")));

        for (key, desc) in keys {
            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!("  {:20}", key), Style::default().fg(Color::Yellow)),
                Span::styled(desc.to_string(), Style::default()),
            ])));
        }

        items.push(ListItem::new(Line::from("")));
    }

    let list = List::new(items).block(block);

    f.render_widget(list, area);
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
