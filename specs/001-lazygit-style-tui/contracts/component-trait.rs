// Component Trait Contract
// This defines the interface for all TUI components

use crossterm::event::Event;
use ratatui::{Frame, layout::Rect};

/// Trait for all TUI components.
/// Each component manages its own state and rendering.
pub trait Component {
    /// Render the component to the given area.
    /// Called on every frame - immediate mode rendering.
    fn render(&mut self, f: &mut Frame, area: Rect);

    /// Handle an input event.
    /// Returns true if the event was consumed, false to propagate.
    fn handle_event(&mut self, event: &Event) -> bool;

    /// Handle an action (from async operations or internal updates).
    fn update(&mut self, action: &Action);

    /// Get the component's title for display in borders.
    fn title(&self) -> &str;

    /// Check if the component is currently focused.
    fn is_focused(&self) -> bool;

    /// Set focus state.
    fn set_focused(&mut self, focused: bool);
}

/// Trait for components that support keyboard navigation.
pub trait Navigable: Component {
    /// Move selection up.
    fn up(&mut self);

    /// Move selection down.
    fn down(&mut self);

    /// Get current selection index.
    fn selected_index(&self) -> Option<usize>;

    /// Get total item count.
    fn item_count(&self) -> usize;
}

/// Trait for components that support selection toggling.
pub trait Selectable: Navigable {
    /// Toggle selection of current item.
    fn toggle_current(&mut self);

    /// Select all items.
    fn select_all(&mut self);

    /// Deselect all items.
    fn deselect_all(&mut self);

    /// Get selected item indices.
    fn selected_indices(&self) -> Vec<usize>;
}
