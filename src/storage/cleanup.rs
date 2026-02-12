use std::fs;

use chrono::{Duration, Utc};

use super::StorageManager;

const RETENTION_HOURS: i64 = 48;
const MIN_CLEANUP_INTERVAL_SECS: i64 = 3600; // 1 hour

impl StorageManager {
    /// Run cleanup if enough time has passed since last cleanup.
    pub fn maybe_auto_cleanup(&self) {
        let analytics = self.read_analytics();
        if let Some(last) = analytics.last_cleanup {
            let elapsed = Utc::now().signed_duration_since(last);
            if elapsed.num_seconds() < MIN_CLEANUP_INTERVAL_SECS {
                return;
            }
        }
        let _ = self.run_cleanup();
    }

    /// Delete logs older than retention period. Returns number of entries cleaned.
    pub fn run_cleanup(&self) -> std::io::Result<usize> {
        let cutoff = Utc::now() - Duration::hours(RETENTION_HOURS);
        let mut cleaned = 0;

        let logs_dir = self.logs_dir();
        if !logs_dir.exists() {
            return Ok(0);
        }

        for entry in fs::read_dir(&logs_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(log_entry) = serde_json::from_str::<crate::types::LogEntry>(&content) {
                    if log_entry.timestamp < cutoff {
                        let ref_id = &log_entry.ref_id;
                        let log_path = self.logs_dir().join(format!("{ref_id}.log"));
                        let _ = fs::remove_file(&path);
                        let _ = fs::remove_file(&log_path);
                        cleaned += 1;
                    }
                }
            }
        }

        // Clean old session files
        let sessions_dir = self.sessions_dir();
        if sessions_dir.exists() {
            for entry in fs::read_dir(&sessions_dir)? {
                let entry = entry?;
                let path = entry.path();
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        let age = modified.elapsed().unwrap_or_default();
                        if age.as_secs() > (RETENTION_HOURS as u64 * 3600) {
                            let _ = fs::remove_file(&path);
                        }
                    }
                }
            }
        }

        // Update last_cleanup timestamp
        let mut analytics = self.read_analytics();
        analytics.last_cleanup = Some(Utc::now());
        let _ = self.write_analytics(&analytics);

        Ok(cleaned)
    }
}
