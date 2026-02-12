use std::fs;
use std::io::Write;

use chrono::Utc;

use crate::types::SessionEntry;

use super::StorageManager;

impl StorageManager {
    fn session_id() -> String {
        Utc::now().format("%Y-%m-%d").to_string()
    }

    fn session_path(&self) -> std::path::PathBuf {
        self.sessions_dir()
            .join(format!("{}.json", Self::session_id()))
    }

    pub fn append_session_entry(&self, entry: &SessionEntry) -> std::io::Result<()> {
        let path = self.session_path();
        let mut entries = self.read_session_entries()?;
        entries.push(entry.clone());

        let json = serde_json::to_string_pretty(&entries)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let tmp_path = path.with_extension("json.tmp");
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(json.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp_path, &path)?;

        Ok(())
    }

    pub fn read_session_entries(&self) -> std::io::Result<Vec<SessionEntry>> {
        let path = self.session_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&path)?;
        serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}
