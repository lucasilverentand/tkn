pub mod analytics;
pub mod cleanup;
pub mod log_store;
pub mod plugin_store;
pub mod session;

use std::fs;
use std::path::PathBuf;

pub struct StorageManager {
    pub base_dir: PathBuf,
}

impl StorageManager {
    pub fn new() -> Self {
        let base_dir = dirs::home_dir()
            .expect("cannot determine home directory")
            .join(".tkn");
        Self { base_dir }
    }

    pub fn init(&self) -> std::io::Result<()> {
        fs::create_dir_all(self.logs_dir())?;
        fs::create_dir_all(self.sessions_dir())?;
        fs::create_dir_all(self.tools_dir())?;
        Ok(())
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.base_dir.join("logs")
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.base_dir.join("sessions")
    }

    pub fn tools_dir(&self) -> PathBuf {
        self.base_dir.join("tools")
    }

    pub fn analytics_path(&self) -> PathBuf {
        self.base_dir.join("analytics.json")
    }
}
