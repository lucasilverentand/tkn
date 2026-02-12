use std::fs;
use std::io;
use std::path::PathBuf;

use crate::types::PluginManifest;

use super::StorageManager;

impl StorageManager {
    pub fn plugins_manifest_path(&self) -> PathBuf {
        self.base_dir.join("plugins.json")
    }

    pub fn read_plugin_manifest(&self) -> PluginManifest {
        let path = self.plugins_manifest_path();
        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => PluginManifest::default(),
        }
    }

    pub fn write_plugin_manifest(&self, manifest: &PluginManifest) -> io::Result<()> {
        let path = self.plugins_manifest_path();
        let tmp = path.with_extension("json.tmp");
        let content = serde_json::to_string_pretty(manifest)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        fs::write(&tmp, &content)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }
}
