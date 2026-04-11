//! Progress component
//!
//! Displays execution progress with a progress bar and statistics.

use crossterm::event::Event;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

use super::Component;
use crate::tui::state::ExecutionProgress;

/// Progress display component
pub struct ProgressComponent {
    /// Current progress state
    progress: Option<ExecutionProgress>,
}

impl Default for ProgressComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressComponent {
    /// Create a new progress component
    pub fn new() -> Self {
        Self { progress: None }
    }

    /// Set the progress state
    pub fn set_progress(&mut self, progress: ExecutionProgress) {
        self.progress = Some(progress);
    }

    /// Clear the progress state
    pub fn clear(&mut self) {
        self.progress = None;
    }

    /// Check if there's active progress
    pub fn is_active(&self) -> bool {
        self.progress.is_some()
    }
}

impl Component for ProgressComponent {
    fn render(&self, f: &mut Frame, area: Rect, focused: bool) {
        let border_style = if focused {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Execution Progress ");

        let inner_area = block.inner(area);
        f.render_widget(block, area);

        if let Some(ref progress) = self.progress {
            // Split into sections
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3), // Progress bar
                    Constraint::Length(1), // Spacer
                    Constraint::Length(2), // Current file
                    Constraint::Length(1), // Spacer
                    Constraint::Min(0),    // Statistics
                ])
                .split(inner_area);

            // Progress bar
            let percentage = progress.percentage();
            let label = format!(
                "{}/{} ({:.1}%)",
                progress.processed_files,
                progress.total_files,
                percentage * 100.0
            );

            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
                .percent((percentage * 100.0) as u16)
                .label(label);

            f.render_widget(gauge, chunks[0]);

            // Current file info
            let current_info = if let Some(ref file) = progress.current_file {
                let filename = file
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| file.display().to_string());
                let op = progress
                    .current_operation
                    .as_deref()
                    .unwrap_or("Processing");
                Line::from(vec![
                    Span::styled(format!("{}: ", op), Style::default().fg(Color::DarkGray)),
                    Span::styled(filename, Style::default().fg(Color::White)),
                ])
            } else {
                Line::from(Span::styled(
                    "Preparing...",
                    Style::default().fg(Color::DarkGray),
                ))
            };

            let current_para = Paragraph::new(current_info);
            f.render_widget(current_para, chunks[2]);

            // Statistics
            let stats_lines = vec![
                Line::from(vec![
                    Span::styled("Success: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        progress.success_count.to_string(),
                        Style::default().fg(Color::Green),
                    ),
                    Span::raw("  "),
                    Span::styled("Errors: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        progress.error_count.to_string(),
                        Style::default().fg(Color::Red),
                    ),
                    Span::raw("  "),
                    Span::styled("Skipped: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        progress.skip_count.to_string(),
                        Style::default().fg(Color::Yellow),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Elapsed: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(progress.elapsed_string(), Style::default().fg(Color::Cyan)),
                    Span::raw("  "),
                    Span::styled("ETA: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(progress.eta_string(), Style::default().fg(Color::Cyan)),
                ]),
            ];

            let stats_para = Paragraph::new(stats_lines);
            f.render_widget(stats_para, chunks[4]);
        } else {
            // No progress - show placeholder
            let text = Paragraph::new(Line::from(Span::styled(
                "No operation in progress",
                Style::default().fg(Color::DarkGray),
            )));
            f.render_widget(text, inner_area);
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }

    fn title(&self) -> &str {
        "Progress"
    }
}
