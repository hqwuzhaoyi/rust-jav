//! TUI Components module
//!
//! Contains reusable UI components for the three-panel layout.

pub mod dialogs;
pub mod file_tree;
pub mod log_viewer;
pub mod operations;
pub mod preview;
pub mod progress;

pub use dialogs::render_dialog;
pub use file_tree::FileTreeComponent;
pub use log_viewer::LogViewerComponent;
pub use operations::OperationsComponent;
pub use preview::PreviewComponent;
pub use progress::ProgressComponent;

use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

/// Trait for all TUI components.
/// Each component manages its own state and rendering.
pub trait Component {
    /// Render the component to the given area.
    fn render(&self, f: &mut Frame, area: Rect, focused: bool);

    /// Handle an input event.
    /// Returns true if the event was consumed.
    fn handle_event(&mut self, event: &Event) -> bool;

    /// Get the component's title for display in borders.
    fn title(&self) -> &str;
}
