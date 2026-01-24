//! Tests for TUI execution engine components
//!
//! Tests ExecutionProgress, LogEntry, and App state transitions.

use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::mpsc;

use rust_jav::tui::app::{App, AppMode};
use rust_jav::tui::state::{ExecutionProgress, LogEntry, LogLevel};

/// Test ExecutionProgress::new() creates correctly
#[test]
fn test_execution_progress_new() {
    let progress = ExecutionProgress::new(10);

    assert_eq!(progress.total_files, 10);
    assert_eq!(progress.processed_files, 0);
    assert_eq!(progress.success_count, 0);
    assert_eq!(progress.error_count, 0);
    assert_eq!(progress.skip_count, 0);
    assert_eq!(progress.current_file, None);
    assert_eq!(progress.current_operation, None);
    assert!(!progress.paused);
    assert!(!progress.cancelled);
}

/// Test ExecutionProgress::new() with zero files
#[test]
fn test_execution_progress_new_zero_files() {
    let progress = ExecutionProgress::new(0);

    assert_eq!(progress.total_files, 0);
    assert_eq!(progress.processed_files, 0);
}

/// Test ExecutionProgress::is_complete() when processed >= total
#[test]
fn test_execution_progress_is_complete_when_done() {
    let mut progress = ExecutionProgress::new(3);

    // Should not be complete initially
    assert!(!progress.is_complete());

    // Process files
    progress.record_success();
    assert!(!progress.is_complete());

    progress.record_error();
    assert!(!progress.is_complete());

    progress.record_skip();
    assert!(progress.is_complete(), "Should be complete when processed equals total");
}

/// Test ExecutionProgress::is_complete() when cancelled
#[test]
fn test_execution_progress_is_complete_when_cancelled() {
    let mut progress = ExecutionProgress::new(10);

    assert!(!progress.is_complete());

    progress.cancel();
    assert!(progress.is_complete(), "Should be complete when cancelled");
}

/// Test ExecutionProgress::is_complete() when processed > total
#[test]
fn test_execution_progress_is_complete_when_exceeded() {
    let mut progress = ExecutionProgress::new(2);

    progress.record_success();
    progress.record_success();
    progress.record_success(); // One more than total

    assert!(progress.is_complete(), "Should be complete when processed exceeds total");
}

/// Test ExecutionProgress::percentage() calculation
#[test]
fn test_execution_progress_percentage() {
    let mut progress = ExecutionProgress::new(10);

    assert_eq!(progress.percentage(), 0.0);

    progress.record_success();
    assert_eq!(progress.percentage(), 0.1);

    progress.record_success();
    progress.record_success();
    progress.record_success();
    progress.record_success();
    assert_eq!(progress.percentage(), 0.5);

    // Complete all
    for _ in 5..10 {
        progress.record_success();
    }
    assert_eq!(progress.percentage(), 1.0);
}

/// Test ExecutionProgress::percentage() with zero files
#[test]
fn test_execution_progress_percentage_zero_files() {
    let progress = ExecutionProgress::new(0);
    assert_eq!(progress.percentage(), 1.0, "Zero files should be 100% complete");
}

/// Test ExecutionProgress::percentage_int() calculation
#[test]
fn test_execution_progress_percentage_int() {
    let mut progress = ExecutionProgress::new(10);

    assert_eq!(progress.percentage_int(), 0);

    progress.record_success();
    assert_eq!(progress.percentage_int(), 10);

    for _ in 1..5 {
        progress.record_success();
    }
    assert_eq!(progress.percentage_int(), 50);

    for _ in 5..10 {
        progress.record_success();
    }
    assert_eq!(progress.percentage_int(), 100);
}

/// Test ExecutionProgress::record_success()
#[test]
fn test_execution_progress_record_success() {
    let mut progress = ExecutionProgress::new(10);

    progress.record_success();

    assert_eq!(progress.processed_files, 1);
    assert_eq!(progress.success_count, 1);
    assert_eq!(progress.error_count, 0);
    assert_eq!(progress.skip_count, 0);
}

/// Test ExecutionProgress::record_error()
#[test]
fn test_execution_progress_record_error() {
    let mut progress = ExecutionProgress::new(10);

    progress.record_error();

    assert_eq!(progress.processed_files, 1);
    assert_eq!(progress.success_count, 0);
    assert_eq!(progress.error_count, 1);
    assert_eq!(progress.skip_count, 0);
}

/// Test ExecutionProgress::record_skip()
#[test]
fn test_execution_progress_record_skip() {
    let mut progress = ExecutionProgress::new(10);

    progress.record_skip();

    assert_eq!(progress.processed_files, 1);
    assert_eq!(progress.success_count, 0);
    assert_eq!(progress.error_count, 0);
    assert_eq!(progress.skip_count, 1);
}

/// Test ExecutionProgress::set_current()
#[test]
fn test_execution_progress_set_current() {
    let mut progress = ExecutionProgress::new(10);

    let file = PathBuf::from("/test/file.mp4");
    let operation = "Rename".to_string();

    progress.set_current(file.clone(), operation.clone());

    assert_eq!(progress.current_file, Some(file));
    assert_eq!(progress.current_operation, Some(operation));
}

/// Test ExecutionProgress::toggle_pause()
#[test]
fn test_execution_progress_toggle_pause() {
    let mut progress = ExecutionProgress::new(10);

    assert!(!progress.paused);

    progress.toggle_pause();
    assert!(progress.paused);

    progress.toggle_pause();
    assert!(!progress.paused);
}

/// Test ExecutionProgress::cancel()
#[test]
fn test_execution_progress_cancel() {
    let mut progress = ExecutionProgress::new(10);

    assert!(!progress.cancelled);

    progress.cancel();
    assert!(progress.cancelled);
}

/// Test ExecutionProgress::elapsed()
#[test]
fn test_execution_progress_elapsed() {
    let progress = ExecutionProgress::new(10);

    std::thread::sleep(Duration::from_millis(100));

    let elapsed = progress.elapsed();
    assert!(elapsed >= Duration::from_millis(100));
}

/// Test mixed operations recording
#[test]
fn test_execution_progress_mixed_operations() {
    let mut progress = ExecutionProgress::new(10);

    progress.record_success();
    progress.record_success();
    progress.record_error();
    progress.record_skip();
    progress.record_success();

    assert_eq!(progress.processed_files, 5);
    assert_eq!(progress.success_count, 3);
    assert_eq!(progress.error_count, 1);
    assert_eq!(progress.skip_count, 1);
    assert_eq!(progress.percentage_int(), 50);
}

/// Test LogEntry::info() creates correct log level
#[test]
fn test_log_entry_info() {
    let entry = LogEntry::info("Test info message");

    assert_eq!(entry.level, LogLevel::Info);
    assert_eq!(entry.message, "Test info message");
    assert_eq!(entry.file, None);
}

/// Test LogEntry::success() creates correct log level
#[test]
fn test_log_entry_success() {
    let entry = LogEntry::success("Test success message");

    assert_eq!(entry.level, LogLevel::Success);
    assert_eq!(entry.message, "Test success message");
    assert_eq!(entry.file, None);
}

/// Test LogEntry::warning() creates correct log level
#[test]
fn test_log_entry_warning() {
    let entry = LogEntry::warning("Test warning message");

    assert_eq!(entry.level, LogLevel::Warning);
    assert_eq!(entry.message, "Test warning message");
    assert_eq!(entry.file, None);
}

/// Test LogEntry::error() creates correct log level
#[test]
fn test_log_entry_error() {
    let entry = LogEntry::error("Test error message");

    assert_eq!(entry.level, LogLevel::Error);
    assert_eq!(entry.message, "Test error message");
    assert_eq!(entry.file, None);
}

/// Test LogEntry::with_file() adds file path
#[test]
fn test_log_entry_with_file() {
    let file = PathBuf::from("/test/file.mp4");
    let entry = LogEntry::info("Test message").with_file(file.clone());

    assert_eq!(entry.level, LogLevel::Info);
    assert_eq!(entry.message, "Test message");
    assert_eq!(entry.file, Some(file));
}

/// Test LogEntry creation with String types
#[test]
fn test_log_entry_string_types() {
    let message = String::from("Dynamic message");
    let entry = LogEntry::info(message.clone());

    assert_eq!(entry.message, message);
}

/// Test App::start_execution() transitions to Executing mode
#[test]
fn test_app_start_execution() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let source_dir = PathBuf::from("/test/source");
    let mut app = App::new(source_dir, tx);

    assert_eq!(app.mode, AppMode::Normal);
    assert!(app.execution.is_none());

    app.start_execution();

    assert_eq!(app.mode, AppMode::Executing);
    assert!(app.execution.is_some());

    let execution = app.execution.as_ref().unwrap();
    assert_eq!(execution.processed_files, 0);
}

/// Test App::complete_execution() returns to Normal mode
#[test]
fn test_app_complete_execution() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let source_dir = PathBuf::from("/test/source");
    let mut app = App::new(source_dir, tx);

    // Start execution first
    app.start_execution();
    assert_eq!(app.mode, AppMode::Executing);
    assert!(app.execution.is_some());

    // Complete execution
    app.complete_execution();

    assert_eq!(app.mode, AppMode::Normal);
    assert!(app.execution.is_none());
}

/// Test App::cancel_execution() returns to Normal mode
#[test]
fn test_app_cancel_execution() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let source_dir = PathBuf::from("/test/source");
    let mut app = App::new(source_dir, tx);

    // Start execution first
    app.start_execution();
    assert_eq!(app.mode, AppMode::Executing);
    assert!(app.execution.is_some());

    // Cancel execution
    app.cancel_execution();

    assert_eq!(app.mode, AppMode::Normal);
    assert!(app.execution.is_none());
}

/// Test App::add_log() adds entries correctly
#[test]
fn test_app_add_log() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let source_dir = PathBuf::from("/test/source");
    let mut app = App::new(source_dir, tx);

    assert_eq!(app.logs.len(), 0);

    app.add_log(LogEntry::info("First log"));
    assert_eq!(app.logs.len(), 1);

    app.add_log(LogEntry::success("Second log"));
    assert_eq!(app.logs.len(), 2);

    app.add_log(LogEntry::error("Third log"));
    assert_eq!(app.logs.len(), 3);
}

/// Test App::add_log() respects 1000 entry limit
#[test]
fn test_app_add_log_limit() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let source_dir = PathBuf::from("/test/source");
    let mut app = App::new(source_dir, tx);

    // Add 1100 entries
    for i in 0..1100 {
        app.add_log(LogEntry::info(format!("Log entry {}", i)));
    }

    // Should keep only last 1000
    assert_eq!(app.logs.len(), 1000);

    // First entry should be #100 (0-99 were removed)
    assert_eq!(app.logs.front().unwrap().message, "Log entry 100");

    // Last entry should be #1099
    assert_eq!(app.logs.back().unwrap().message, "Log entry 1099");
}

/// Test App initial state
#[test]
fn test_app_initial_state() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let source_dir = PathBuf::from("/test/source");
    let app = App::new(source_dir.clone(), tx);

    assert_eq!(app.mode, AppMode::Normal);
    assert!(app.execution.is_none());
    assert!(app.dialog.is_none());
    assert_eq!(app.logs.len(), 0);
    assert!(!app.should_quit);
    assert_eq!(app.source_dir, source_dir);
    assert_eq!(app.search_query, "");
}

/// Test App::enter_search() and exit_search()
#[test]
fn test_app_search_mode() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let source_dir = PathBuf::from("/test/source");
    let mut app = App::new(source_dir, tx);

    assert_eq!(app.mode, AppMode::Normal);
    assert_eq!(app.search_query, "");

    app.enter_search();
    assert_eq!(app.mode, AppMode::Search);

    // Simulate typing search query
    app.search_query = "test query".to_string();

    app.exit_search();
    assert_eq!(app.mode, AppMode::Normal);
    assert_eq!(app.search_query, "");
}

/// Test App state transitions: Normal -> Executing -> Normal
#[test]
fn test_app_execution_cycle() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let source_dir = PathBuf::from("/test/source");
    let mut app = App::new(source_dir, tx);

    // Initial state
    assert_eq!(app.mode, AppMode::Normal);
    assert!(app.execution.is_none());

    // Start execution
    app.start_execution();
    assert_eq!(app.mode, AppMode::Executing);
    assert!(app.execution.is_some());

    // Verify execution progress is initialized
    let execution = app.execution.as_ref().unwrap();
    assert_eq!(execution.processed_files, 0);
    assert!(!execution.cancelled);

    // Complete execution
    app.complete_execution();
    assert_eq!(app.mode, AppMode::Normal);
    assert!(app.execution.is_none());
}

/// Test App state transitions: Normal -> Executing -> Cancelled -> Normal
#[test]
fn test_app_execution_cancel_cycle() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let source_dir = PathBuf::from("/test/source");
    let mut app = App::new(source_dir, tx);

    // Initial state
    assert_eq!(app.mode, AppMode::Normal);

    // Start execution
    app.start_execution();
    assert_eq!(app.mode, AppMode::Executing);

    // Cancel execution
    app.cancel_execution();
    assert_eq!(app.mode, AppMode::Normal);
    assert!(app.execution.is_none());
}

/// Test ExecutionProgress with realistic scenario
#[test]
fn test_execution_progress_realistic_scenario() {
    let mut progress = ExecutionProgress::new(5);

    // File 1: Success
    progress.set_current(PathBuf::from("/test/file1.mp4"), "Rename".to_string());
    progress.record_success();
    assert_eq!(progress.processed_files, 1);
    assert_eq!(progress.success_count, 1);
    assert!(!progress.is_complete());

    // File 2: Success
    progress.set_current(PathBuf::from("/test/file2.mp4"), "Rename".to_string());
    progress.record_success();
    assert_eq!(progress.processed_files, 2);
    assert_eq!(progress.success_count, 2);
    assert!(!progress.is_complete());

    // File 3: Error
    progress.set_current(PathBuf::from("/test/file3.mp4"), "Rename".to_string());
    progress.record_error();
    assert_eq!(progress.processed_files, 3);
    assert_eq!(progress.error_count, 1);
    assert!(!progress.is_complete());

    // File 4: Skip
    progress.set_current(PathBuf::from("/test/file4.mp4"), "Rename".to_string());
    progress.record_skip();
    assert_eq!(progress.processed_files, 4);
    assert_eq!(progress.skip_count, 1);
    assert!(!progress.is_complete());

    // File 5: Success
    progress.set_current(PathBuf::from("/test/file5.mp4"), "Rename".to_string());
    progress.record_success();
    assert_eq!(progress.processed_files, 5);
    assert_eq!(progress.success_count, 3);
    assert!(progress.is_complete());

    // Final counts
    assert_eq!(progress.success_count, 3);
    assert_eq!(progress.error_count, 1);
    assert_eq!(progress.skip_count, 1);
    assert_eq!(progress.percentage_int(), 100);
}

/// Test LogEntry timestamp is set
#[test]
fn test_log_entry_timestamp() {
    let entry = LogEntry::info("Test");
    let now = chrono::Local::now();

    // Timestamp should be within 1 second of now
    let diff = now.signed_duration_since(entry.timestamp);
    assert!(diff.num_seconds().abs() <= 1);
}

/// Test Default trait for ExecutionProgress
#[test]
fn test_execution_progress_default() {
    let progress = ExecutionProgress::default();

    assert_eq!(progress.total_files, 0);
    assert_eq!(progress.processed_files, 0);
    assert_eq!(progress.success_count, 0);
    assert_eq!(progress.error_count, 0);
    assert_eq!(progress.skip_count, 0);
}
