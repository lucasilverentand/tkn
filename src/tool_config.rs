use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::types::normalize_tool;

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
}

/// Returns all built-in plugins as (name, toml_content) pairs.
pub fn builtin_plugins() -> Vec<(&'static str, &'static str)> {
    vec![
        ("git-diff", include_str!("../plugins/git-diff.toml")),
        ("git-log", include_str!("../plugins/git-log.toml")),
        ("git-status", include_str!("../plugins/git-status.toml")),
        ("git-show", include_str!("../plugins/git-show.toml")),
        ("git-blame", include_str!("../plugins/git-blame.toml")),
        ("git-branch", include_str!("../plugins/git-branch.toml")),
        ("git-stash", include_str!("../plugins/git-stash.toml")),
        ("cargo-build", include_str!("../plugins/cargo-build.toml")),
        ("cargo-test", include_str!("../plugins/cargo-test.toml")),
        ("cargo-clippy", include_str!("../plugins/cargo-clippy.toml")),
    ]
}

fn load_defaults() -> Vec<ToolConfig> {
    builtin_plugins()
        .into_iter()
        .filter_map(|(name, content)| {
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

pub fn load_tool_config(command: &str) -> Option<ToolConfig> {
    let tool_name = normalize_tool(command);

    // User override takes priority
    if let Some(config) = load_user_config(&tool_name) {
        return Some(config);
    }

    // Fall back to built-in defaults
    load_defaults()
        .into_iter()
        .find(|c| c.match_pattern == tool_name)
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
        assert!(!config.optimize.strip.is_empty());
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
        assert_eq!(plugins.len(), 10);
        let names: Vec<&str> = plugins.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"git-diff"));
        assert!(names.contains(&"cargo-build"));
        assert!(names.contains(&"git-blame"));
    }

    #[test]
    fn test_git_blame_has_max_bytes() {
        let config = load_tool_config("git blame src/main.rs").unwrap();
        assert_eq!(config.optimize.max_bytes, Some(16384));
    }
}
