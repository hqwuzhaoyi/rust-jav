//! Operations list component
//!
//! Displays batch operations with toggle checkboxes.

use std::path::PathBuf;

use crossterm::event::Event;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use super::Component;
use crate::tui::state::{Operation, OperationType, OperationsState};

/// Operations panel component
pub struct OperationsComponent {
    /// Operations state
    state: OperationsState,
    /// List state for ratatui
    list_state: ListState,
}

impl Default for OperationsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl OperationsComponent {
    /// Create a new operations component
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            state: OperationsState::new(),
            list_state,
        }
    }

    /// Move to next operation
    pub fn next(&mut self) {
        self.state.next();
        self.list_state.select(Some(self.state.selected));
    }

    /// Move to previous operation
    pub fn previous(&mut self) {
        self.state.previous();
        self.list_state.select(Some(self.state.selected));
    }

    /// Toggle current operation
    pub fn toggle_current(&mut self) {
        self.state.toggle_current();
    }

    /// Toggle all operations
    pub fn toggle_all(&mut self) {
        self.state.toggle_all();
    }

    /// Check if any operations are enabled
    pub fn has_enabled_operations(&self) -> bool {
        self.state.has_enabled()
    }

    /// Get the currently selected operation
    pub fn selected_operation(&self) -> Option<&Operation> {
        self.state.current()
    }

    /// Get all operations
    pub fn operations(&self) -> &[Operation] {
        &self.state.operations
    }

    /// Get enabled operations count
    pub fn enabled_count(&self) -> usize {
        self.state.enabled_count()
    }

    /// Get enabled operation count (alias)
    pub fn enabled_operation_count(&self) -> usize {
        self.state.enabled_count()
    }

    /// Update affected files for a specific operation type
    pub fn update_affected_files(&mut self, op_type: OperationType, files: Vec<PathBuf>) {
        for op in &mut self.state.operations {
            if op.op_type == op_type {
                op.set_affected(files);
                break;
            }
        }
    }

    /// Update affected files for all operations from analysis results
    pub fn update_all_affected(&mut self, results: Vec<(OperationType, Vec<PathBuf>)>) {
        for (op_type, files) in results {
            self.update_affected_files(op_type, files);
        }
    }
}

impl Component for OperationsComponent {
    fn render(&self, f: &mut Frame, area: Rect, focused: bool) {
        let items: Vec<ListItem> = self
            .state
            .operations
            .iter()
            .enumerate()
            .map(|(idx, op)| {
                let checkbox = if op.enabled { "[x]" } else { "[ ]" };
                let count_str = if op.affected_count > 0 {
                    format!(" ({})", op.affected_count)
                } else {
                    String::new()
                };

                let style = if idx == self.state.selected && focused {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::REVERSED | Modifier::BOLD)
                } else if op.enabled {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };

                let checkbox_style = if op.enabled {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let content = Line::from(vec![
                    Span::styled(format!("{} ", checkbox), checkbox_style),
                    Span::styled(op.op_type.name(), style),
                    Span::styled(count_str, Style::default().fg(Color::DarkGray)),
                ]);

                ListItem::new(content)
            })
            .collect();

        let border_style = if focused {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let enabled_count = self.state.enabled_count();
        let title = if enabled_count > 0 {
            format!(
                " Operations [{}/{}] ",
                enabled_count,
                self.state.operations.len()
            )
        } else {
            " Operations ".to_string()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title);

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        let mut state = self.list_state.clone();
        f.render_stateful_widget(list, area, &mut state);
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        // Events handled by main event loop
        false
    }

    fn title(&self) -> &str {
        "Operations"
    }
}
