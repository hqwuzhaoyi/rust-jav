//! Log persistence module (T085-T087)
//!
//! Persists log entries to ~/.rust-jav/logs/ directory.

use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use super::file_state::{LogEntry, LogLevel};

/// Log persistence handler
pub struct LogPersistence {
    /// Log file path
    log_file: PathBuf,
    /// Whether persistence is enabled
    enabled: bool,
}

impl LogPersistence {
    /// Create a new log persistence handler
    pub fn new() -> Self {
        let log_dir = Self::get_log_dir();
        let log_file = Self::get_log_file_path(&log_dir);
        let enabled = Self::ensure_log_dir(&log_dir);

        Self { log_file, enabled }
    }

    /// Get the log directory path (~/.rust-jav/logs/)
    fn get_log_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".rust-jav")
            .join("logs")
    }

    /// Get log file path with date-based naming
    fn get_log_file_path(log_dir: &Path) -> PathBuf {
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        log_dir.join(format!("tui-{}.log", date))
    }

    /// Ensure log directory exists
    fn ensure_log_dir(log_dir: &PathBuf) -> bool {
        if !log_dir.exists() {
            fs::create_dir_all(log_dir).is_ok()
        } else {
            true
        }
    }

    /// Write a log entry to the persistent log file
    pub fn write(&self, entry: &LogEntry) {
        if !self.enabled {
            return;
        }

        let level_str = match entry.level {
            LogLevel::Info => "INFO",
            LogLevel::Success => "SUCCESS",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
        };

        let timestamp = entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f");
        let file_info = entry
            .file
            .as_ref()
            .map(|f| format!(" [{}]", f.display()))
            .unwrap_or_default();

        let line = format!(
            "[{}] [{}]{} {}\n",
            timestamp, level_str, file_info, entry.message
        );

        // Append to log file
        if let Ok(file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file)
        {
            let mut writer = BufWriter::new(file);
            let _ = writer.write_all(line.as_bytes());
            let _ = writer.flush();
        }
    }

    /// Write a session start marker
    pub fn write_session_start(&self, source_dir: &Path) {
        if !self.enabled {
            return;
        }

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let separator = "=".repeat(60);
        let content = format!(
            "\n{}\n[{}] Session started\nSource directory: {}\n{}\n",
            separator,
            timestamp,
            source_dir.display(),
            separator
        );

        if let Ok(file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file)
        {
            let mut writer = BufWriter::new(file);
            let _ = writer.write_all(content.as_bytes());
            let _ = writer.flush();
        }
    }

    /// Write a session end marker
    pub fn write_session_end(&self, stats: SessionStats) {
        if !self.enabled {
            return;
        }

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let content = format!(
            "[{}] Session ended - Success: {}, Errors: {}, Warnings: {}\n",
            timestamp, stats.success_count, stats.error_count, stats.warning_count
        );

        if let Ok(file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file)
        {
            let mut writer = BufWriter::new(file);
            let _ = writer.write_all(content.as_bytes());
            let _ = writer.flush();
        }
    }

    /// Get the log file path
    pub fn log_file_path(&self) -> &PathBuf {
        &self.log_file
    }

    /// Check if persistence is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for LogPersistence {
    fn default() -> Self {
        Self::new()
    }
}

/// Session statistics for end-of-session summary
#[derive(Debug, Default)]
pub struct SessionStats {
    pub success_count: usize,
    pub error_count: usize,
    pub warning_count: usize,
}

impl SessionStats {
    /// Calculate stats from log entries
    pub fn from_logs(logs: impl IntoIterator<Item = impl AsRef<LogEntry>>) -> Self {
        let mut stats = Self::default();
        for entry in logs {
            match entry.as_ref().level {
                LogLevel::Success => stats.success_count += 1,
                LogLevel::Error => stats.error_count += 1,
                LogLevel::Warning => stats.warning_count += 1,
                LogLevel::Info => {}
            }
        }
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_dir_path() {
        let log_dir = LogPersistence::get_log_dir();
        assert!(log_dir.to_string_lossy().contains(".rust-jav/logs"));
    }

    #[test]
    fn test_log_file_naming() {
        let log_dir = PathBuf::from("/tmp/test-logs");
        let log_file = LogPersistence::get_log_file_path(&log_dir);
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        assert!(log_file.to_string_lossy().contains(&date));
    }
}
