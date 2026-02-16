use std::collections::HashSet;
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

    pub fn update_analytics_with_patterns(&self, entry: &LogEntry, patterns: &HashSet<String>) -> std::io::Result<()> {
        let mut analytics = self.read_analytics();

        analytics.total_commands += 1;
        analytics.total_raw_bytes += entry.raw_bytes as u64;
        analytics.total_optimized_bytes += entry.optimized_bytes as u64;
        let estimated = entry.estimated_raw_bytes.unwrap_or(entry.raw_bytes) as u64;
        analytics.total_estimated_raw_bytes += estimated;
        analytics.total_duration_ms += entry.duration_ms;
        analytics.last_updated = Some(Utc::now());

        let tool = normalize_tool(&entry.command, patterns);
        let tool_stats = analytics.tools.entry(tool).or_insert_with(ToolStats::default);
        tool_stats.count += 1;
        tool_stats.total_raw_bytes += entry.raw_bytes as u64;
        tool_stats.total_optimized_bytes += entry.optimized_bytes as u64;
        tool_stats.total_estimated_raw_bytes += estimated;
        if entry.exit_code != 0 {
            tool_stats.failures += 1;
        }
        if entry.transformed_command.is_some() {
            tool_stats.transformations += 1;
        }

        self.write_analytics(&analytics)
    }

    /// Record that a full log was read for a given command (signals optimizer may strip too much).
    pub fn record_full_log_read(&self, command: &str, patterns: &HashSet<String>) -> std::io::Result<()> {
        let mut analytics = self.read_analytics();
        let tool = normalize_tool(command, patterns);
        let tool_stats = analytics.tools.entry(tool).or_insert_with(ToolStats::default);
        tool_stats.full_log_reads += 1;
        self.write_analytics(&analytics)
    }

    /// Remove a specific tool from analytics, subtracting its counts from the totals.
    /// Returns Ok(true) if the tool was found and removed, Ok(false) if not found.
    pub fn reset_tool_stats(&self, tool: &str) -> std::io::Result<bool> {
        let mut analytics = self.read_analytics();
        let Some(stats) = analytics.tools.remove(tool) else {
            return Ok(false);
        };
        analytics.total_commands = analytics.total_commands.saturating_sub(stats.count);
        analytics.total_raw_bytes = analytics.total_raw_bytes.saturating_sub(stats.total_raw_bytes);
        analytics.total_optimized_bytes = analytics.total_optimized_bytes.saturating_sub(stats.total_optimized_bytes);
        analytics.total_estimated_raw_bytes = analytics.total_estimated_raw_bytes.saturating_sub(stats.total_estimated_raw_bytes);
        self.write_analytics(&analytics)?;
        Ok(true)
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
