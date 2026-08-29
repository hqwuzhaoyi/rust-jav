//! rust-jav library
//!
//! Core functionality for JAV file processing and TUI application.

pub mod active_rules;
pub mod actor_links;
pub mod actor_views;
pub mod application;
pub mod asset_index;
pub mod cli;
pub mod config;
#[cfg(unix)]
pub mod deletion_plan;
pub mod file_utils;
pub mod management;
pub mod management_tasks;
pub mod migration_verifier;
pub mod nfo_check;
pub mod operations;
pub mod report;
pub mod runtime;
pub mod tui;
