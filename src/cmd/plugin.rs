use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use chrono::Utc;

use crate::storage::StorageManager;
use crate::tool_config::builtin_plugins;
use crate::types::{PluginEntry, PluginManifest, PluginSource};

const DEFAULT_REPO: &str = "https://github.com/lucasilverentand/tkn";

pub fn install(url: Option<&str>) -> i32 {
    let storage = StorageManager::new();
    if let Err(e) = storage.init() {
        eprintln!("error: failed to initialize storage: {e}");
        return 1;
    }

    let mut manifest = storage.read_plugin_manifest();

    let result = match url {
        None => install_builtins(&storage, &mut manifest),
        Some(u) => install_from_git(u, &storage, &mut manifest),
    };

    if let Err(e) = storage.write_plugin_manifest(&manifest) {
        eprintln!("error: failed to write plugin manifest: {e}");
        return 1;
    }

    result
}

fn install_builtins(storage: &StorageManager, manifest: &mut PluginManifest) -> i32 {
    let tools_dir = storage.tools_dir();
    let mut installed = 0;

    for (bundle, name, content) in builtin_plugins() {
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
                bundle: bundle.to_string(),
                source: PluginSource::Builtin,
                installed_at: Utc::now(),
            });
        }

        println!("  installed: {name}");
        installed += 1;
    }

    println!("{installed} plugin(s) installed to {}", tools_dir.display());
    0
}

fn install_from_git(url: &str, storage: &StorageManager, manifest: &mut PluginManifest) -> i32 {
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
            return 1;
        }
        Err(e) => {
            eprintln!("error: failed to run git: {e}");
            return 1;
        }
    }

    let plugins_dir = tmp_dir.join("plugins");
    if !plugins_dir.exists() {
        eprintln!("error: no plugins/ directory found in repository");
        let _ = fs::remove_dir_all(&tmp_dir);
        return 1;
    }

    let tools_dir = storage.tools_dir();
    let mut installed = 0;

    // Look for bundle subdirectories first
    if let Ok(entries) = fs::read_dir(&plugins_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let bundle = entry.file_name().to_string_lossy().to_string();
                if let Ok(files) = fs::read_dir(&path) {
                    for file in files.flatten() {
                        let file_path = file.path();
                        if file_path.extension().is_some_and(|ext| ext == "toml") {
                            let stem = file_path.file_stem().unwrap().to_string_lossy();
                            let name = format!("{bundle}-{stem}");
                            let dest = tools_dir.join(format!("{name}.toml"));

                            if dest.exists() {
                                println!("  skip: {name} (already exists)");
                                continue;
                            }

                            match fs::copy(&file_path, &dest) {
                                Ok(_) => {
                                    if !manifest.plugins.iter().any(|p| p.name == name) {
                                        manifest.plugins.push(PluginEntry {
                                            name: name.clone(),
                                            bundle: bundle.clone(),
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
            } else if path.extension().is_some_and(|ext| ext == "toml") {
                // Flat plugin files at the top level (backwards compat)
                let file_name = path.file_name().unwrap().to_string_lossy().to_string();
                let name = file_name.trim_end_matches(".toml");
                let bundle = name.split('-').next().unwrap_or(name).to_string();
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
                                bundle,
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
    0
}

pub fn list() {
    let storage = StorageManager::new();
    let manifest = storage.read_plugin_manifest();
    let tools_dir = storage.tools_dir();

    let builtins = builtin_plugins();

    // Group built-in plugins by bundle
    let mut builtin_bundles: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (bundle, name, _) in &builtins {
        builtin_bundles.entry(bundle).or_default().push(name);
    }

    println!("Plugins:");
    println!();

    // Show built-in bundles
    for (bundle, names) in &builtin_bundles {
        let any_installed = names.iter().any(|n| tools_dir.join(format!("{n}.toml")).exists());
        let status = if any_installed { "installed" } else { "built-in" };
        println!("  {bundle} ({status})");

        let short_names: Vec<&str> = names
            .iter()
            .map(|n| n.strip_prefix(&format!("{bundle}-")).unwrap_or(n))
            .collect();
        println!("    {}", short_names.join(", "));
    }

    // Show any extra installed plugins (from git repos), grouped by bundle
    let builtin_names: Vec<&str> = builtins.iter().map(|(_, n, _)| *n).collect();
    let mut extra_bundles: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in &manifest.plugins {
        if builtin_names.contains(&entry.name.as_str()) {
            continue;
        }
        let on_disk = tools_dir.join(format!("{}.toml", entry.name)).exists();
        if on_disk {
            extra_bundles
                .entry(entry.bundle.clone())
                .or_default()
                .push(entry.name.clone());
        }
    }

    for (bundle, names) in &extra_bundles {
        let source = manifest
            .plugins
            .iter()
            .find(|p| p.bundle == *bundle)
            .map(|p| match &p.source {
                PluginSource::Builtin => "built-in".to_string(),
                PluginSource::Git { url } => url.clone(),
            })
            .unwrap_or_default();
        println!("  {bundle} ({source})");

        let short_names: Vec<&str> = names
            .iter()
            .map(|n| n.strip_prefix(&format!("{bundle}-")).unwrap_or(n.as_str()))
            .collect();
        println!("    {}", short_names.join(", "));
    }
}

pub fn remove(name: &str) -> i32 {
    let storage = StorageManager::new();
    let tools_dir = storage.tools_dir();
    let builtins = builtin_plugins();

    // Check if name matches a bundle — remove all plugins in that bundle
    let bundle_plugins: Vec<&str> = builtins
        .iter()
        .filter(|(b, _, _)| *b == name)
        .map(|(_, n, _)| *n)
        .collect();

    if !bundle_plugins.is_empty() {
        let mut removed = 0;
        let mut manifest = storage.read_plugin_manifest();

        for plugin_name in &bundle_plugins {
            let path = tools_dir.join(format!("{plugin_name}.toml"));
            if path.exists() {
                if let Err(e) = fs::remove_file(&path) {
                    eprintln!("  error: failed to remove {plugin_name}: {e}");
                    continue;
                }
                println!("  removed: {plugin_name}");
                removed += 1;
            }
        }

        manifest.plugins.retain(|p| p.bundle != name);
        if let Err(e) = storage.write_plugin_manifest(&manifest) {
            eprintln!("error: failed to update manifest: {e}");
        }

        if removed == 0 {
            println!("{name} bundle: no plugins installed to disk, nothing to remove");
        } else {
            println!("{removed} plugin(s) removed from bundle {name}");
        }
        return 0;
    }

    // Single plugin removal
    let path = tools_dir.join(format!("{name}.toml"));
    if !path.exists() {
        if builtins.iter().any(|(_, n, _)| *n == name) {
            println!("{name} is a built-in plugin (not installed to disk, nothing to remove)");
            return 0;
        } else {
            eprintln!("error: plugin '{name}' not found");
            return 1;
        }
    }

    if let Err(e) = fs::remove_file(&path) {
        eprintln!("error: failed to remove {name}: {e}");
        return 1;
    }

    // Remove from manifest
    let mut manifest = storage.read_plugin_manifest();
    manifest.plugins.retain(|p| p.name != name);
    if let Err(e) = storage.write_plugin_manifest(&manifest) {
        eprintln!("error: failed to update manifest: {e}");
    }

    println!("removed: {name}");
    0
}
