//! TUI module for LazyGit-style terminal user interface
//!
//! This module provides a three-panel TUI for file management operations.

pub mod app;
pub mod components;
pub mod event;
pub mod executor;
pub mod state;
pub mod ui;

pub use app::{App, AppMode, Panel};
pub use event::{init_terminal, restore_terminal, run_app};
