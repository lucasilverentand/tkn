use std::collections::HashMap;
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

    pub max_bytes: Option<usize>,

    /// Skip blank-line collapse and trailing-whitespace trim (only ANSI strip + CR resolve).
    #[serde(default)]
    pub raw: bool,
}

/// Returns all built-in plugins as (bundle, name, toml_content) triples.
pub fn builtin_plugins() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("git", "git-diff", include_str!("../plugins/git/diff.toml")),
        ("git", "git-log", include_str!("../plugins/git/log.toml")),
        ("git", "git-status", include_str!("../plugins/git/status.toml")),
        ("git", "git-show", include_str!("../plugins/git/show.toml")),
        ("git", "git-blame", include_str!("../plugins/git/blame.toml")),
        ("git", "git-branch", include_str!("../plugins/git/branch.toml")),
        ("git", "git-stash", include_str!("../plugins/git/stash.toml")),
        ("cargo", "cargo-build", include_str!("../plugins/cargo/build.toml")),
        ("cargo", "cargo-test", include_str!("../plugins/cargo/test.toml")),
        ("cargo", "cargo-clippy", include_str!("../plugins/cargo/clippy.toml")),
        ("gh", "gh-issue", include_str!("../plugins/gh/issue.toml")),
        ("gh", "gh-pr", include_str!("../plugins/gh/pr.toml")),
        ("gh", "gh-repo", include_str!("../plugins/gh/repo.toml")),
        ("gh", "gh-run", include_str!("../plugins/gh/run.toml")),
        ("gh", "gh-workflow", include_str!("../plugins/gh/workflow.toml")),
        ("gh", "gh-api", include_str!("../plugins/gh/api.toml")),
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
        if let Some(max_bytes) = o.max_bytes {
            config.optimize.max_bytes = Some(max_bytes);
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
    let tool_name = normalize_tool(command);
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
        assert!(config.transform.add.contains(&"--no-color".to_string()));
    }

    #[test]
    fn test_load_default_cargo_build() {
        let config = load_tool_config("cargo build --release").unwrap();
        assert_eq!(config.match_pattern, "cargo build");
        assert!(config.transform.add.contains(&"--color=never".to_string()));
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
        assert!(names.contains(&"git log"));
        assert!(names.contains(&"git status"));
        assert!(names.contains(&"git show"));
        assert!(names.contains(&"git blame"));
        assert!(names.contains(&"git branch"));
        assert!(names.contains(&"git stash"));
        assert!(names.contains(&"cargo build"));
        assert!(names.contains(&"cargo test"));
        assert!(names.contains(&"cargo clippy"));
    }

    #[test]
    fn test_builtin_plugins_returns_all() {
        let plugins = builtin_plugins();
        assert_eq!(plugins.len(), 16);
        let names: Vec<&str> = plugins.iter().map(|(_, n, _)| *n).collect();
        assert!(names.contains(&"git-diff"));
        assert!(names.contains(&"cargo-build"));
        assert!(names.contains(&"git-blame"));
        assert!(names.contains(&"gh-issue"));
        assert!(names.contains(&"gh-pr"));
        assert!(names.contains(&"gh-api"));
    }

    #[test]
    fn test_git_blame_has_max_bytes() {
        let config = load_tool_config("git blame src/main.rs").unwrap();
        assert_eq!(config.optimize.max_bytes, Some(16384));
    }

    #[test]
    fn test_builtin_plugins_have_bundles() {
        let plugins = builtin_plugins();
        let git_plugins: Vec<_> = plugins.iter().filter(|(b, _, _)| *b == "git").collect();
        assert_eq!(git_plugins.len(), 7);
        let cargo_plugins: Vec<_> = plugins.iter().filter(|(b, _, _)| *b == "cargo").collect();
        assert_eq!(cargo_plugins.len(), 3);
        let gh_plugins: Vec<_> = plugins.iter().filter(|(b, _, _)| *b == "gh").collect();
        assert_eq!(gh_plugins.len(), 6);
    }

    #[test]
    fn test_apply_overrides_max_bytes() {
        let mut config = ToolConfig {
            match_pattern: "git blame".to_string(),
            optimize: OptimizeConfig {
                max_bytes: Some(16384),
                ..Default::default()
            },
            ..Default::default()
        };
        let overrides = PluginOverrides {
            optimize: Some(crate::types::OptimizeOverrides {
                max_bytes: Some(32768),
                ..Default::default()
            }),
            ..Default::default()
        };
        apply_overrides(&mut config, &overrides);
        assert_eq!(config.optimize.max_bytes, Some(32768));
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
