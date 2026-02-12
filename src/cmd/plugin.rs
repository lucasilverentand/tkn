use std::fs;
use std::process::Command;

use chrono::Utc;

use crate::storage::StorageManager;
use crate::tool_config::builtin_plugins;
use crate::types::{PluginEntry, PluginManifest, PluginSource};

const DEFAULT_REPO: &str = "https://github.com/lucasilverentand/tkn";

pub fn install(url: Option<&str>) {
    let storage = StorageManager::new();
    if let Err(e) = storage.init() {
        eprintln!("error: failed to initialize storage: {e}");
        return;
    }

    let mut manifest = storage.read_plugin_manifest();

    match url {
        None => install_builtins(&storage, &mut manifest),
        Some(u) => install_from_git(u, &storage, &mut manifest),
    }

    if let Err(e) = storage.write_plugin_manifest(&manifest) {
        eprintln!("error: failed to write plugin manifest: {e}");
    }
}

fn install_builtins(storage: &StorageManager, manifest: &mut PluginManifest) {
    let tools_dir = storage.tools_dir();
    let mut installed = 0;

    for (name, content) in builtin_plugins() {
        let path = tools_dir.join(format!("{name}.toml"));
        if path.exists() {
            println!("  skip: {name} (already exists)");
            continue;
        }
        if let Err(e) = fs::write(&path, content) {
            eprintln!("  error: {name}: {e}");
            continue;
        }

        // Add to manifest if not already tracked
        if !manifest.plugins.iter().any(|p| p.name == name) {
            manifest.plugins.push(PluginEntry {
                name: name.to_string(),
                source: PluginSource::Builtin,
                installed_at: Utc::now(),
            });
        }

        println!("  installed: {name}");
        installed += 1;
    }

    println!("{installed} plugin(s) installed to {}", tools_dir.display());
}

fn install_from_git(url: &str, storage: &StorageManager, manifest: &mut PluginManifest) {
    let url = if url == "default" { DEFAULT_REPO } else { url };

    let tmp_dir = std::env::temp_dir().join(format!("tkn-plugin-{}", uuid::Uuid::new_v4()));

    println!("cloning {url}...");
    let status = Command::new("git")
        .args(["clone", "--depth", "1", url, tmp_dir.to_str().unwrap()])
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            eprintln!("error: git clone failed with exit code {}", s.code().unwrap_or(-1));
            return;
        }
        Err(e) => {
            eprintln!("error: failed to run git: {e}");
            return;
        }
    }

    let plugins_dir = tmp_dir.join("plugins");
    if !plugins_dir.exists() {
        eprintln!("error: no plugins/ directory found in repository");
        let _ = fs::remove_dir_all(&tmp_dir);
        return;
    }

    let tools_dir = storage.tools_dir();
    let mut installed = 0;

    if let Ok(entries) = fs::read_dir(&plugins_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                let file_name = path.file_name().unwrap().to_string_lossy().to_string();
                let name = file_name.trim_end_matches(".toml");
                let dest = tools_dir.join(&file_name);

                if dest.exists() {
                    println!("  skip: {name} (already exists)");
                    continue;
                }

                match fs::copy(&path, &dest) {
                    Ok(_) => {
                        if !manifest.plugins.iter().any(|p| p.name == name) {
                            manifest.plugins.push(PluginEntry {
                                name: name.to_string(),
                                source: PluginSource::Git { url: url.to_string() },
                                installed_at: Utc::now(),
                            });
                        }
                        println!("  installed: {name}");
                        installed += 1;
                    }
                    Err(e) => eprintln!("  error: {name}: {e}"),
                }
            }
        }
    }

    let _ = fs::remove_dir_all(&tmp_dir);
    println!("{installed} plugin(s) installed from {url}");
}

pub fn list() {
    let storage = StorageManager::new();
    let manifest = storage.read_plugin_manifest();
    let tools_dir = storage.tools_dir();

    let builtins = builtin_plugins();
    let builtin_names: Vec<&str> = builtins.iter().map(|(n, _)| *n).collect();

    println!("Plugins:");
    println!();

    // Show built-in plugins
    for name in &builtin_names {
        let on_disk = tools_dir.join(format!("{name}.toml")).exists();
        let status = if on_disk { "installed" } else { "built-in" };
        println!("  {name}  ({status})");
    }

    // Show any extra installed plugins (from git repos)
    for entry in &manifest.plugins {
        if builtin_names.contains(&entry.name.as_str()) {
            continue;
        }
        let source = match &entry.source {
            PluginSource::Builtin => "built-in".to_string(),
            PluginSource::Git { url } => url.clone(),
        };
        let on_disk = tools_dir.join(format!("{}.toml", entry.name)).exists();
        if on_disk {
            println!("  {}  ({})", entry.name, source);
        }
    }
}

pub fn remove(name: &str) {
    let storage = StorageManager::new();
    let tools_dir = storage.tools_dir();
    let path = tools_dir.join(format!("{name}.toml"));

    if !path.exists() {
        // Check if it's a built-in
        let builtins = builtin_plugins();
        if builtins.iter().any(|(n, _)| *n == name) {
            println!("{name} is a built-in plugin (not installed to disk, nothing to remove)");
        } else {
            eprintln!("error: plugin '{name}' not found");
        }
        return;
    }

    if let Err(e) = fs::remove_file(&path) {
        eprintln!("error: failed to remove {name}: {e}");
        return;
    }

    // Remove from manifest
    let mut manifest = storage.read_plugin_manifest();
    manifest.plugins.retain(|p| p.name != name);
    if let Err(e) = storage.write_plugin_manifest(&manifest) {
        eprintln!("error: failed to update manifest: {e}");
    }

    println!("removed: {name}");
}
