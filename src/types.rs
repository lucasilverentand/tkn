use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub ref_id: String,
    pub command: String,
    pub exit_code: i32,
    pub raw_bytes: usize,
    pub optimized_bytes: usize,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub ref_id: String,
    pub command: String,
    pub exit_code: i32,
    pub raw_bytes: usize,
    pub optimized_bytes: usize,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Analytics {
    pub total_commands: u64,
    pub total_raw_bytes: u64,
    pub total_optimized_bytes: u64,
    pub total_duration_ms: u64,
    pub last_updated: Option<DateTime<Utc>>,
    pub last_cleanup: Option<DateTime<Utc>>,
}

impl Analytics {
    pub fn savings_percent(&self) -> f64 {
        if self.total_raw_bytes == 0 {
            return 0.0;
        }
        let saved = self.total_raw_bytes.saturating_sub(self.total_optimized_bytes);
        (saved as f64 / self.total_raw_bytes as f64) * 100.0
    }
}

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct OptimizedOutput {
    pub content: String,
    pub original_bytes: usize,
    pub optimized_bytes: usize,
    pub was_truncated: bool,
}
