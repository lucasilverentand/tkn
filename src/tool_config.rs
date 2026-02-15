use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::storage::StorageManager;
use crate::types::{normalize_tool, PluginOverrides, Settings};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ToolConfig {
    #[serde(rename = "match")]
    pub match_pattern: String,

    #[serde(default)]
    pub transform: TransformConfig,

    #[serde(default)]
    pub optimize: OptimizeConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TransformConfig {
    #[serde(default)]
    pub add: Vec<String>,

    #[serde(default)]
    pub remove: Vec<String>,

    #[serde(default)]
    pub replace: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct OptimizeConfig {
    #[serde(default)]
    pub strip: Vec<String>,

    #[serde(default)]
    pub keep: Vec<String>,

    pub max_lines: Option<usize>,

    /// Skip blank-line collapse and trailing-whitespace trim (only ANSI strip + CR resolve).
    #[serde(default)]
    pub raw: bool,
}

/// Returns all built-in plugins as (bundle, name, toml_content) triples.
pub fn builtin_plugins() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("git", "git-diff", include_str!("../plugins/git/diff.toml")),
        ("git", "git-status", include_str!("../plugins/git/status.toml")),
        ("git", "git-show", include_str!("../plugins/git/show.toml")),
        ("cargo", "cargo-build", include_str!("../plugins/cargo/build.toml")),
        ("cargo", "cargo-test", include_str!("../plugins/cargo/test.toml")),
        ("cargo", "cargo-clippy", include_str!("../plugins/cargo/clippy.toml")),
        ("gh", "gh-issue", include_str!("../plugins/gh/issue.toml")),
        ("gh", "gh-pr", include_str!("../plugins/gh/pr.toml")),
        ("gh", "gh-repo", include_str!("../plugins/gh/repo.toml")),
        ("gh", "gh-run", include_str!("../plugins/gh/run.toml")),
        ("gh", "gh-api", include_str!("../plugins/gh/api.toml")),
        ("find", "find", include_str!("../plugins/find/find.toml")),
        ("curl", "curl", include_str!("../plugins/curl/curl.toml")),
        ("tree", "tree", include_str!("../plugins/tree/tree.toml")),
        ("ls", "ls", include_str!("../plugins/ls/ls.toml")),
        ("git", "git-log", include_str!("../plugins/git/log.toml")),
        ("xcodebuild", "xcodebuild-test", include_str!("../plugins/xcodebuild/test.toml")),
        ("xcodebuild", "xcodebuild-build", include_str!("../plugins/xcodebuild/build.toml")),
        ("bun", "bun-run", include_str!("../plugins/bun/run.toml")),
        ("bun", "bun-test", include_str!("../plugins/bun/test.toml")),
    ]
}

fn load_defaults() -> Vec<ToolConfig> {
    builtin_plugins()
        .into_iter()
        .filter_map(|(_, name, content)| {
            toml::from_str(content)
                .map_err(|e| eprintln!("warning: failed to parse built-in plugin {name}: {e}"))
                .ok()
        })
        .collect()
}

/// Collect all match patterns from builtin + user plugins into a set.
pub fn collect_patterns() -> HashSet<String> {
    let mut patterns = HashSet::new();
    // Builtin patterns
    for config in load_defaults() {
        patterns.insert(config.match_pattern);
    }
    // User-override patterns
    let dir = user_tools_dir();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "toml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(config) = toml::from_str::<ToolConfig>(&content) {
                        patterns.insert(config.match_pattern);
                    }
                }
            }
        }
    }
    patterns
}

fn user_tools_dir() -> PathBuf {
    dirs::home_dir()
        .expect("cannot determine home directory")
        .join(".tkn")
        .join("tools")
}

fn load_user_config(tool_name: &str) -> Option<ToolConfig> {
    let file_name = format!("{}.toml", tool_name.replace(' ', "-"));
    let path = user_tools_dir().join(&file_name);
    let content = fs::read_to_string(path).ok()?;
    toml::from_str(&content).ok()
}

pub fn load_settings() -> Settings {
    let storage = StorageManager::new();
    let path = storage.settings_path();
    match fs::read_to_string(path) {
        Ok(content) => toml::from_str(&content).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

fn apply_overrides(config: &mut ToolConfig, overrides: &PluginOverrides) {
    if let Some(ref t) = overrides.transform {
        if !t.add.is_empty() {
            config.transform.add = t.add.clone();
        }
        if !t.remove.is_empty() {
            config.transform.remove = t.remove.clone();
        }
        if !t.replace.is_empty() {
            config.transform.replace = t.replace.clone();
        }
    }
    if let Some(ref o) = overrides.optimize {
        if let Some(max_lines) = o.max_lines {
            config.optimize.max_lines = Some(max_lines);
        }
        if let Some(ref strip) = o.strip {
            config.optimize.strip = strip.clone();
        }
        if let Some(ref keep) = o.keep {
            config.optimize.keep = keep.clone();
        }
    }
}

pub fn load_tool_config(command: &str) -> Option<ToolConfig> {
    let patterns = collect_patterns();
    load_tool_config_with_patterns(command, &patterns)
}

pub fn load_tool_config_with_patterns(command: &str, patterns: &HashSet<String>) -> Option<ToolConfig> {
    let tool_name = normalize_tool(command, patterns);
    let settings = load_settings();

    // Derive the plugin key (e.g. "git diff" -> "git-diff")
    let plugin_key = tool_name.replace(' ', "-");

    // Check if disabled in settings
    if let Some(overrides) = settings.overrides.get(&plugin_key) {
        if overrides.enabled == Some(false) {
            return None;
        }
    }

    // User override takes priority
    let mut config = if let Some(config) = load_user_config(&tool_name) {
        config
    } else {
        // Fall back to built-in defaults
        load_defaults()
            .into_iter()
            .find(|c| c.match_pattern == tool_name)?
    };

    // Apply settings overrides
    if let Some(overrides) = settings.overrides.get(&plugin_key) {
        apply_overrides(&mut config, overrides);
    }

    Some(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_default_git_diff() {
        let config = load_tool_config("git diff src/main.rs").unwrap();
        assert_eq!(config.match_pattern, "git diff");
        assert!(!config.optimize.strip.is_empty());
    }

    #[test]
    fn test_load_default_cargo_build() {
        let config = load_tool_config("cargo build --release").unwrap();
        assert_eq!(config.match_pattern, "cargo build");
        assert!(!config.optimize.keep.is_empty());
        assert!(!config.optimize.keep.is_empty());
    }

    #[test]
    fn test_no_config_for_unknown() {
        assert!(load_tool_config("echo hello").is_none());
    }

    #[test]
    fn test_defaults_cover_expected_tools() {
        let defaults = load_defaults();
        let names: Vec<&str> = defaults.iter().map(|c| c.match_pattern.as_str()).collect();
        assert!(names.contains(&"git diff"));
        assert!(names.contains(&"git status"));
        assert!(names.contains(&"git show"));
        assert!(names.contains(&"cargo build"));
        assert!(names.contains(&"cargo test"));
        assert!(names.contains(&"cargo clippy"));
    }

    #[test]
    fn test_builtin_plugins_returns_all() {
        let plugins = builtin_plugins();
        assert_eq!(plugins.len(), 20);
        let names: Vec<&str> = plugins.iter().map(|(_, n, _)| *n).collect();
        assert!(names.contains(&"git-diff"));
        assert!(names.contains(&"cargo-build"));
        assert!(names.contains(&"gh-issue"));
        assert!(names.contains(&"gh-pr"));
        assert!(names.contains(&"gh-api"));
    }

    #[test]
    fn test_builtin_plugins_have_bundles() {
        let plugins = builtin_plugins();
        let git_plugins: Vec<_> = plugins.iter().filter(|(b, _, _)| *b == "git").collect();
        assert_eq!(git_plugins.len(), 4);
        let cargo_plugins: Vec<_> = plugins.iter().filter(|(b, _, _)| *b == "cargo").collect();
        assert_eq!(cargo_plugins.len(), 3);
        let gh_plugins: Vec<_> = plugins.iter().filter(|(b, _, _)| *b == "gh").collect();
        assert_eq!(gh_plugins.len(), 5);
    }

    #[test]
    fn test_apply_overrides_max_lines() {
        let mut config = ToolConfig {
            match_pattern: "git diff".to_string(),
            optimize: OptimizeConfig {
                max_lines: Some(200),
                ..Default::default()
            },
            ..Default::default()
        };
        let overrides = PluginOverrides {
            optimize: Some(crate::types::OptimizeOverrides {
                max_lines: Some(1000),
                ..Default::default()
            }),
            ..Default::default()
        };
        apply_overrides(&mut config, &overrides);
        assert_eq!(config.optimize.max_lines, Some(1000));
    }

    #[test]
    fn test_apply_overrides_transform() {
        let mut config = ToolConfig {
            match_pattern: "git log".to_string(),
            transform: TransformConfig {
                add: vec!["--no-color".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        let overrides = PluginOverrides {
            transform: Some(TransformConfig {
                add: vec!["--no-color".to_string(), "--oneline".to_string()],
                ..Default::default()
            }),
            ..Default::default()
        };
        apply_overrides(&mut config, &overrides);
        assert_eq!(config.transform.add.len(), 2);
        assert!(config.transform.add.contains(&"--oneline".to_string()));
    }
}
