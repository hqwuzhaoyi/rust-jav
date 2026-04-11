//! File tree component
//!
//! Displays a navigable tree view of files and directories.

use std::path::PathBuf;

use crossterm::event::Event;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use super::Component;
use crate::tui::state::{FileNode, FileStatus, TreeState};

/// File tree component for displaying and navigating files
pub struct FileTreeComponent {
    /// Root directory path
    root: PathBuf,
    /// Tree nodes (flattened for display)
    nodes: Vec<FileNode>,
    /// Tree navigation state
    state: TreeState,
    /// List state for ratatui
    list_state: ListState,
    /// Search filter (if any)
    filter: Option<String>,
}

impl FileTreeComponent {
    /// Create a new file tree component
    pub fn new(root: PathBuf) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            root,
            nodes: Vec::new(),
            state: TreeState::new(),
            list_state,
            filter: None,
        }
    }

    /// Scan the root directory and populate the tree (only top level initially)
    pub async fn scan_directory(&mut self) {
        self.nodes.clear();
        if let Ok(nodes) = scan_directory_flat(&self.root, 0).await {
            self.nodes = nodes;
        }
        self.state.selected = 0;
        self.list_state.select(Some(0));
    }

    /// Get the currently selected node
    pub fn selected_node(&self) -> Option<&FileNode> {
        self.nodes.get(self.state.selected)
    }

    /// Get the currently selected path
    pub fn selected_path(&self) -> Option<PathBuf> {
        self.selected_node().map(|n| n.path.clone())
    }

    /// Move selection to next item
    pub fn next(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        self.state.selected = (self.state.selected + 1).min(self.nodes.len() - 1);
        self.list_state.select(Some(self.state.selected));
    }

    /// Move selection to previous item
    pub fn previous(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        self.state.selected = self.state.selected.saturating_sub(1);
        self.list_state.select(Some(self.state.selected));
    }

    /// Expand a directory or enter it
    pub fn expand_or_enter(&mut self) {
        if let Some(node) = self.nodes.get(self.state.selected).cloned() {
            if node.is_dir {
                let path = node.path.clone();
                if self.state.is_expanded(&path) {
                    // Already expanded, do nothing or enter
                } else {
                    self.state.toggle_expanded(&path);
                    // Trigger lazy load
                    self.load_children_sync(self.state.selected);
                }
            }
        }
    }

    /// Collapse a directory or go back to parent
    pub fn collapse_or_back(&mut self) {
        if let Some(node) = self.nodes.get(self.state.selected).cloned() {
            if node.is_dir && self.state.is_expanded(&node.path) {
                self.state.toggle_expanded(&node.path);
                self.remove_children(self.state.selected);
            } else if node.depth > 0 {
                // Go to parent
                self.go_to_parent();
            }
        }
    }

    /// Go to parent directory
    fn go_to_parent(&mut self) {
        if let Some(node) = self.nodes.get(self.state.selected) {
            let current_depth = node.depth;
            if current_depth == 0 {
                return;
            }
            // Find parent node
            for i in (0..self.state.selected).rev() {
                if self.nodes[i].depth < current_depth {
                    self.state.selected = i;
                    self.list_state.select(Some(i));
                    return;
                }
            }
        }
    }

    /// Load children synchronously (placeholder - should be async)
    fn load_children_sync(&mut self, parent_idx: usize) {
        if parent_idx >= self.nodes.len() {
            return;
        }
        let parent = &self.nodes[parent_idx];
        if !parent.is_dir || parent.children_loaded {
            return;
        }

        let parent_path = parent.path.clone();
        let parent_depth = parent.depth;

        // Mark as loaded
        if let Some(node) = self.nodes.get_mut(parent_idx) {
            node.children_loaded = true;
        }

        // Synchronously read directory (for now)
        if let Ok(entries) = std::fs::read_dir(parent_path) {
            let mut children: Vec<FileNode> = entries
                .filter_map(|e| e.ok())
                .map(|entry| {
                    let path = entry.path();
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let is_dir = path.is_dir();
                    FileNode::new(name, path, is_dir, parent_depth + 1)
                })
                .collect();

            // Sort: directories first, then alphabetically
            children.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            });

            // Insert children after parent
            let insert_pos = parent_idx + 1;
            for (i, child) in children.into_iter().enumerate() {
                self.nodes.insert(insert_pos + i, child);
            }
        }
    }

    /// Remove children of a collapsed directory
    fn remove_children(&mut self, parent_idx: usize) {
        if parent_idx >= self.nodes.len() {
            return;
        }
        let parent_depth = self.nodes[parent_idx].depth;

        // Remove all nodes after parent with greater depth
        let mut remove_count = 0;
        for i in (parent_idx + 1)..self.nodes.len() {
            if self.nodes[i].depth > parent_depth {
                remove_count += 1;
            } else {
                break;
            }
        }

        if remove_count > 0 {
            self.nodes
                .drain((parent_idx + 1)..(parent_idx + 1 + remove_count));
        }

        // Mark as not loaded
        if let Some(node) = self.nodes.get_mut(parent_idx) {
            node.children_loaded = false;
        }
    }

    /// Toggle multi-select mode
    pub fn toggle_multi_select(&mut self) {
        self.state.multi_select_mode = !self.state.multi_select_mode;
        if !self.state.multi_select_mode {
            self.state.clear_selections();
        }
    }

    /// Check if multi-select mode is active
    pub fn is_multi_select(&self) -> bool {
        self.state.multi_select_mode
    }

    /// Toggle selection of current item
    pub fn toggle_current_selection(&mut self) {
        if let Some(node) = self.nodes.get(self.state.selected) {
            let path = node.path.clone();
            self.state.toggle_selection(&path);
        }
    }

    /// Select or deselect all files (not directories)
    pub fn toggle_select_all(&mut self) {
        // Check if all files are already selected
        let file_paths: Vec<_> = self
            .nodes
            .iter()
            .filter(|n| !n.is_dir)
            .map(|n| n.path.clone())
            .collect();

        let all_selected = file_paths.iter().all(|p| self.state.is_selected(p));

        if all_selected {
            // Deselect all
            self.state.clear_selections();
        } else {
            // Select all files
            for path in file_paths {
                if !self.state.is_selected(&path) {
                    self.state.toggle_selection(&path);
                }
            }
        }
    }

    /// Get the count of selected files
    pub fn selected_count(&self) -> usize {
        self.state.selected_files.len()
    }

    /// Get all selected file paths
    pub fn selected_files(&self) -> Vec<PathBuf> {
        self.state.selected_files.iter().cloned().collect()
    }

    /// Filter tree by search query
    pub fn filter_by_query(&mut self, query: &str) {
        self.filter = if query.is_empty() {
            None
        } else {
            Some(query.to_lowercase())
        };
    }

    /// Apply search and reset filter
    pub fn apply_search(&mut self, query: &str) {
        if query.is_empty() {
            return;
        }
        let query_lower = query.to_lowercase();
        // Find first matching node
        for (i, node) in self.nodes.iter().enumerate() {
            if node.name.to_lowercase().contains(&query_lower) {
                self.state.selected = i;
                self.list_state.select(Some(i));
                break;
            }
        }
        self.filter = None;
    }

    /// Get the total number of nodes (T099)
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get visible nodes (filtered if search active)
    fn visible_nodes(&self) -> Vec<(usize, &FileNode)> {
        match &self.filter {
            Some(query) => self
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| n.name.to_lowercase().contains(query))
                .collect(),
            None => self.nodes.iter().enumerate().collect(),
        }
    }
}

impl Component for FileTreeComponent {
    fn render(&self, f: &mut Frame, area: Rect, focused: bool) {
        // Use visible_nodes() to apply search filter (T109 fix)
        let visible = self.visible_nodes();

        // T098: Handle empty directory with friendly message
        if self.nodes.is_empty() {
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
                .title(" Files ");

            let empty_message = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "📂 Directory is empty",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "No files found in this directory.",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(Span::styled(
                    "Try selecting a different source directory.",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .block(block)
            .alignment(ratatui::layout::Alignment::Center);

            f.render_widget(empty_message, area);
            return;
        }

        let items: Vec<ListItem> = visible
            .iter()
            .map(|(original_idx, node)| {
                let indent = "  ".repeat(node.depth);
                let icon = if node.is_dir {
                    if self.state.is_expanded(&node.path) {
                        "📂 "
                    } else {
                        "📁 "
                    }
                } else {
                    "📄 "
                };

                let selected_marker = if self.state.is_selected(&node.path) {
                    "✓ "
                } else {
                    ""
                };

                let style = match node.status {
                    FileStatus::ToMove => Style::default().fg(Color::Yellow),
                    FileStatus::ToDelete => Style::default().fg(Color::Red),
                    FileStatus::ToRename => Style::default().fg(Color::Cyan),
                    FileStatus::Selected => Style::default().fg(Color::Green),
                    FileStatus::Unchanged => Style::default(),
                };

                let content = format!("{}{}{}{}", indent, selected_marker, icon, node.name);

                let style = if *original_idx == self.state.selected && focused {
                    style.add_modifier(Modifier::REVERSED)
                } else {
                    style
                };

                ListItem::new(Line::from(Span::styled(content, style)))
            })
            .collect();

        let border_style = if focused {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let title = if self.filter.is_some() {
            format!(" Files [SEARCH: {} matches] ", visible.len())
        } else if self.state.multi_select_mode {
            format!(
                " Files [MULTI-SELECT: {} selected] ",
                self.state.selected_files.len()
            )
        } else {
            " Files ".to_string()
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
        "Files"
    }
}

/// Scan a directory and return a flat list of nodes (non-recursive, only top level)
async fn scan_directory_flat(path: &PathBuf, depth: usize) -> std::io::Result<Vec<FileNode>> {
    let mut nodes = Vec::new();

    let mut entries = tokio::fs::read_dir(path).await?;
    let mut items = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let entry_path = entry.path();
        let name = entry_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let is_dir = entry_path.is_dir();
        items.push((name, entry_path, is_dir));
    }

    // Sort: directories first, then alphabetically
    items.sort_by(|a, b| match (a.2, b.2) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
    });

    for (name, item_path, is_dir) in items {
        let node = FileNode::new(name, item_path, is_dir, depth);
        nodes.push(node);
    }

    Ok(nodes)
}
