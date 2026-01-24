//! Log viewer component
//!
//! Displays scrollable log entries during and after execution.

use std::collections::VecDeque;

use crossterm::event::Event;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use super::Component;
use crate::tui::state::{LogEntry, LogLevel};

/// Log viewer component
pub struct LogViewerComponent {
    /// Log entries
    entries: VecDeque<LogEntry>,
    /// Maximum number of entries to keep
    max_entries: usize,
    /// List state for scrolling
    list_state: ListState,
    /// Auto-scroll to bottom
    auto_scroll: bool,
}

impl Default for LogViewerComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl LogViewerComponent {
    /// Create a new log viewer
    pub fn new() -> Self {
        Self {
            entries: VecDeque::with_capacity(1000),
            max_entries: 1000,
            list_state: ListState::default(),
            auto_scroll: true,
        }
    }

    /// Add a log entry
    pub fn add_entry(&mut self, entry: LogEntry) {
        self.entries.push_back(entry);
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }
        if self.auto_scroll {
            self.list_state.select(Some(self.entries.len().saturating_sub(1)));
        }
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
        self.list_state.select(None);
    }

    /// Scroll up
    pub fn scroll_up(&mut self) {
        self.auto_scroll = false;
        if let Some(selected) = self.list_state.selected() {
            self.list_state.select(Some(selected.saturating_sub(1)));
        } else if !self.entries.is_empty() {
            self.list_state.select(Some(self.entries.len() - 1));
        }
    }

    /// Scroll down
    pub fn scroll_down(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            let new_idx = (selected + 1).min(self.entries.len().saturating_sub(1));
            self.list_state.select(Some(new_idx));
            // Re-enable auto-scroll if at bottom
            if new_idx == self.entries.len().saturating_sub(1) {
                self.auto_scroll = true;
            }
        }
    }

    /// Toggle auto-scroll
    pub fn toggle_auto_scroll(&mut self) {
        self.auto_scroll = !self.auto_scroll;
        if self.auto_scroll && !self.entries.is_empty() {
            self.list_state.select(Some(self.entries.len() - 1));
        }
    }

    /// Get entry count
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

impl Component for LogViewerComponent {
    fn render(&self, f: &mut Frame, area: Rect, focused: bool) {
        let border_style = if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let auto_scroll_indicator = if self.auto_scroll { " [AUTO]" } else { "" };
        let title = format!(" Logs ({} entries){} ", self.entries.len(), auto_scroll_indicator);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title);

        let items: Vec<ListItem> = self.entries
            .iter()
            .map(|entry| {
                let timestamp = entry.timestamp.format("%H:%M:%S").to_string();
                let level_style = match entry.level {
                    LogLevel::Info => Style::default().fg(Color::Blue),
                    LogLevel::Success => Style::default().fg(Color::Green),
                    LogLevel::Warning => Style::default().fg(Color::Yellow),
                    LogLevel::Error => Style::default().fg(Color::Red),
                };
                let level_str = match entry.level {
                    LogLevel::Info => "INFO ",
                    LogLevel::Success => "OK   ",
                    LogLevel::Warning => "WARN ",
                    LogLevel::Error => "ERROR",
                };

                let mut spans = vec![
                    Span::styled(
                        format!("{} ", timestamp),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(level_str, level_style),
                    Span::raw(" "),
                    Span::styled(&entry.message, Style::default()),
                ];

                if let Some(ref file) = entry.file {
                    let filename = file.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| file.display().to_string());
                    spans.push(Span::styled(
                        format!(" [{}]", filename),
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut state = self.list_state.clone();
        f.render_stateful_widget(list, area, &mut state);
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }

    fn title(&self) -> &str {
        "Logs"
    }
}
