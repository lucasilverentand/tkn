use std::cmp::Reverse;
use std::fs;

use crate::types::LogEntry;

use super::StorageManager;

impl StorageManager {
    pub fn write_log(
        &self,
        ref_id: &str,
        raw_output: &[u8],
        entry: &LogEntry,
    ) -> std::io::Result<()> {
        let log_path = self.logs_dir().join(format!("{ref_id}.log"));
        let meta_path = self.logs_dir().join(format!("{ref_id}.json"));

        fs::write(&log_path, raw_output)?;

        let json = serde_json::to_string_pretty(entry).map_err(std::io::Error::other)?;
        fs::write(&meta_path, json)?;

        Ok(())
    }

    pub fn read_log(&self, ref_id: &str) -> std::io::Result<String> {
        let log_path = self.logs_dir().join(format!("{ref_id}.log"));
        fs::read_to_string(&log_path)
    }

    pub fn read_log_entry(&self, ref_id: &str) -> std::io::Result<LogEntry> {
        let meta_path = self.logs_dir().join(format!("{ref_id}.json"));
        let content = fs::read_to_string(&meta_path)?;
        serde_json::from_str(&content).map_err(std::io::Error::other)
    }

    pub fn list_log_entries(&self) -> std::io::Result<Vec<LogEntry>> {
        let mut entries = Vec::new();
        let logs_dir = self.logs_dir();
        if !logs_dir.exists() {
            return Ok(entries);
        }
        for entry in fs::read_dir(&logs_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(log_entry) = serde_json::from_str::<LogEntry>(&content) {
                        entries.push(log_entry);
                    }
                }
            }
        }
        entries.sort_by_key(|entry| Reverse(entry.timestamp));
        Ok(entries)
    }
}
