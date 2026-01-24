//! Execution state management
//!
//! Contains data structures for tracking operation execution progress.

use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Progress tracking for operation execution
#[derive(Debug, Clone)]
pub struct ExecutionProgress {
    /// Total number of files to process
    pub total_files: u64,
    /// Number of files processed so far
    pub processed_files: u64,
    /// Number of successful operations
    pub success_count: u64,
    /// Number of failed operations
    pub error_count: u64,
    /// Number of skipped operations
    pub skip_count: u64,
    /// Currently processing file path
    pub current_file: Option<PathBuf>,
    /// Current operation being performed
    pub current_operation: Option<String>,
    /// Start time of execution
    pub start_time: Instant,
    /// Estimated time remaining
    pub eta: Option<Duration>,
    /// Whether execution is paused
    pub paused: bool,
    /// Whether execution was cancelled
    pub cancelled: bool,
}

impl Default for ExecutionProgress {
    fn default() -> Self {
        Self::new(0)
    }
}

impl ExecutionProgress {
    /// Create a new execution progress tracker
    pub fn new(total_files: u64) -> Self {
        Self {
            total_files,
            processed_files: 0,
            success_count: 0,
            error_count: 0,
            skip_count: 0,
            current_file: None,
            current_operation: None,
            start_time: Instant::now(),
            eta: None,
            paused: false,
            cancelled: false,
        }
    }

    /// Calculate progress percentage (0.0 to 1.0)
    pub fn percentage(&self) -> f64 {
        if self.total_files == 0 {
            1.0
        } else {
            self.processed_files as f64 / self.total_files as f64
        }
    }

    /// Get progress as integer percentage (0-100)
    pub fn percentage_int(&self) -> u8 {
        (self.percentage() * 100.0).round() as u8
    }

    /// Update the current file being processed
    pub fn set_current(&mut self, file: PathBuf, operation: String) {
        self.current_file = Some(file);
        self.current_operation = Some(operation);
    }

    /// Record a successful operation
    pub fn record_success(&mut self) {
        self.processed_files += 1;
        self.success_count += 1;
        self.update_eta();
    }

    /// Record a failed operation
    pub fn record_error(&mut self) {
        self.processed_files += 1;
        self.error_count += 1;
        self.update_eta();
    }

    /// Record a skipped operation
    pub fn record_skip(&mut self) {
        self.processed_files += 1;
        self.skip_count += 1;
        self.update_eta();
    }

    /// Update ETA based on current progress
    fn update_eta(&mut self) {
        if self.processed_files == 0 {
            self.eta = None;
            return;
        }

        let elapsed = self.start_time.elapsed();
        let rate = self.processed_files as f64 / elapsed.as_secs_f64();
        if rate > 0.0 {
            // Handle case where processed might exceed total
            let remaining = self.total_files.saturating_sub(self.processed_files);
            let eta_secs = remaining as f64 / rate;
            self.eta = Some(Duration::from_secs_f64(eta_secs));
        }
    }

    /// Get elapsed time since start
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Format elapsed time as string
    pub fn elapsed_string(&self) -> String {
        format_duration(self.elapsed())
    }

    /// Format ETA as string
    pub fn eta_string(&self) -> String {
        match self.eta {
            Some(eta) => format_duration(eta),
            None => "calculating...".to_string(),
        }
    }

    /// Check if execution is complete
    pub fn is_complete(&self) -> bool {
        self.processed_files >= self.total_files || self.cancelled
    }

    /// Toggle pause state
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    /// Cancel execution
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
}

/// Format a duration as human-readable string
fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}:{:02}", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

/// Result of a single file operation
#[derive(Debug, Clone)]
pub enum FileOperationResult {
    /// Operation completed successfully
    Success {
        file: PathBuf,
        operation: String,
        details: Option<String>,
    },
    /// Operation failed
    Error {
        file: PathBuf,
        operation: String,
        error: String,
    },
    /// Operation was skipped
    Skipped {
        file: PathBuf,
        operation: String,
        reason: String,
    },
}

impl FileOperationResult {
    /// Get the file path for this result
    pub fn file(&self) -> &PathBuf {
        match self {
            FileOperationResult::Success { file, .. } => file,
            FileOperationResult::Error { file, .. } => file,
            FileOperationResult::Skipped { file, .. } => file,
        }
    }

    /// Check if this is a success result
    pub fn is_success(&self) -> bool {
        matches!(self, FileOperationResult::Success { .. })
    }

    /// Check if this is an error result
    pub fn is_error(&self) -> bool {
        matches!(self, FileOperationResult::Error { .. })
    }
}
