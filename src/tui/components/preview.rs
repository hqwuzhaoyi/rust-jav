//! Preview panel component
//!
//! Displays detailed information about selected files or operations.

use crossterm::event::Event;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use std::path::PathBuf;

use super::Component;
use crate::tui::state::{FileNode, Operation};

/// Preview panel component
pub struct PreviewComponent {
    /// Selected files for preview
    selected_files: Vec<PathBuf>,
    /// Total size of selected files
    total_size: u64,
    /// Currently highlighted operation (if any)
    highlighted_operation: Option<Operation>,
    /// Scroll offset for long content
    scroll_offset: u16,
}

impl Default for PreviewComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewComponent {
    /// Create a new preview component
    pub fn new() -> Self {
        Self {
            selected_files: Vec::new(),
            total_size: 0,
            highlighted_operation: None,
            scroll_offset: 0,
        }
    }

    /// Set the selected files to preview
    pub fn set_selected_files(&mut self, files: Vec<PathBuf>) {
        // Calculate total size
        self.total_size = files.iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();
        self.selected_files = files;
        self.scroll_offset = 0;
    }

    /// Set preview to show a single file (convenience wrapper)
    pub fn set_file_preview(&mut self, node: FileNode) {
        if !node.is_dir {
            self.set_selected_files(vec![node.path]);
        }
    }

    /// Set preview to show multiple selected files (convenience wrapper)
    pub fn set_selected_files_preview(&mut self, files: Vec<PathBuf>) {
        self.set_selected_files(files);
    }

    /// Set the highlighted operation (shown below files)
    pub fn set_highlighted_operation(&mut self, operation: Option<Operation>) {
        self.highlighted_operation = operation;
    }

    /// Scroll down
    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    /// Scroll up
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Build the full preview content
    fn build_content(&self) -> Text<'static> {
        let mut lines = Vec::new();

        // Always show selected files section first
        if self.selected_files.is_empty() {
            lines.push(Line::from(Span::styled(
                "No files selected",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            // Header
            lines.push(Line::from(vec![
                Span::styled("📁 Selected Files", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::from(""));

            // Summary
            lines.push(Line::from(vec![
                Span::styled("Count: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    self.selected_files.len().to_string(),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" files", Style::default().fg(Color::DarkGray)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Total Size: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format_file_size(self.total_size),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ),
            ]));

            // File list (up to 15 files to leave room for operation details)
            if !self.selected_files.is_empty() {
                lines.push(Line::from(""));
                let max_files = if self.highlighted_operation.is_some() { 10 } else { 20 };

                for (i, file) in self.selected_files.iter().take(max_files).enumerate() {
                    let filename = file.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| file.display().to_string());

                    let size_str = std::fs::metadata(file)
                        .map(|m| format_file_size(m.len()))
                        .unwrap_or_else(|_| "?".to_string());

                    lines.push(Line::from(vec![
                        Span::styled(format!("  {:2}. ", i + 1), Style::default().fg(Color::DarkGray)),
                        Span::styled(filename, Style::default().fg(Color::White)),
                        Span::styled(format!("  ({})", size_str), Style::default().fg(Color::DarkGray)),
                    ]));
                }

                if self.selected_files.len() > max_files {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("  ... and {} more", self.selected_files.len() - max_files),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));
                }
            }
        }

        // Show highlighted operation details below (if any)
        if let Some(ref op) = self.highlighted_operation {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("─".repeat(30), Style::default().fg(Color::DarkGray)),
            ]));
            lines.push(Line::from(""));

            // Operation header
            lines.push(Line::from(vec![
                Span::styled("⚙️  ", Style::default()),
                Span::styled(
                    op.op_type.name().to_string(),
                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    if op.enabled { "[ON]" } else { "[OFF]" }.to_string(),
                    Style::default().fg(if op.enabled { Color::Green } else { Color::Red }),
                ),
            ]));

            // Description
            lines.push(Line::from(vec![
                Span::styled(op.op_type.description().to_string(), Style::default().fg(Color::DarkGray)),
            ]));

            // Affected count
            if op.affected_count > 0 {
                lines.push(Line::from(vec![
                    Span::styled("Will affect: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{} files", op.affected_count),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(" (", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format_file_size(op.affected_size),
                        Style::default().fg(Color::Green),
                    ),
                    Span::styled(")", Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

        Text::from(lines)
    }
}

impl Component for PreviewComponent {
    fn render(&self, f: &mut Frame, area: Rect, focused: bool) {
        let border_style = if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Preview ");

        let content = self.build_content();

        let paragraph = Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));

        f.render_widget(paragraph, area);
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        // Events handled by main event loop
        false
    }

    fn title(&self) -> &str {
        "Preview"
    }
}

/// Format file size in human-readable format
fn format_file_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} B", size)
    }
}
