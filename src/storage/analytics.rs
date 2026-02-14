use std::fs;
use std::io::Write;

use chrono::Utc;

use crate::types::{Analytics, LogEntry, ToolStats, normalize_tool};

use super::StorageManager;

impl StorageManager {
    pub fn read_analytics(&self) -> Analytics {
        let path = self.analytics_path();
        if !path.exists() {
            return Analytics::default();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn update_analytics(&self, entry: &LogEntry) -> std::io::Result<()> {
        let mut analytics = self.read_analytics();

        analytics.total_commands += 1;
        analytics.total_raw_bytes += entry.raw_bytes as u64;
        analytics.total_optimized_bytes += entry.optimized_bytes as u64;
        analytics.total_duration_ms += entry.duration_ms;
        analytics.last_updated = Some(Utc::now());

        let tool = normalize_tool(&entry.command);
        let tool_stats = analytics.tools.entry(tool).or_insert_with(ToolStats::default);
        tool_stats.count += 1;
        tool_stats.total_raw_bytes += entry.raw_bytes as u64;
        tool_stats.total_optimized_bytes += entry.optimized_bytes as u64;
        if entry.exit_code != 0 {
            tool_stats.failures += 1;
        }

        self.write_analytics(&analytics)
    }

    /// Record that a full log was read for a given command (signals optimizer may strip too much).
    pub fn record_full_log_read(&self, command: &str) -> std::io::Result<()> {
        let mut analytics = self.read_analytics();
        let tool = normalize_tool(command);
        let tool_stats = analytics.tools.entry(tool).or_insert_with(ToolStats::default);
        tool_stats.full_log_reads += 1;
        self.write_analytics(&analytics)
    }

    pub fn write_analytics(&self, analytics: &Analytics) -> std::io::Result<()> {
        let path = self.analytics_path();
        let json = serde_json::to_string_pretty(analytics)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Atomic write via temp file + rename
        let tmp_path = path.with_extension("json.tmp");
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(json.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp_path, &path)?;

        Ok(())
    }
}
