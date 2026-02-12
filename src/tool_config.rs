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

fn load_defaults() -> Vec<ToolConfig> {
    vec![
        ToolConfig {
            match_pattern: "git diff".into(),
            transform: TransformConfig {
                add: vec!["--no-color".into()],
                ..Default::default()
            },
            optimize: OptimizeConfig::default(),
        },
        ToolConfig {
            match_pattern: "git log".into(),
            transform: TransformConfig {
                add: vec!["--no-color".into()],
                ..Default::default()
            },
            optimize: OptimizeConfig::default(),
        },
        ToolConfig {
            match_pattern: "git status".into(),
            transform: TransformConfig {
                add: vec!["--no-color".into()],
                ..Default::default()
            },
            optimize: OptimizeConfig::default(),
        },
        ToolConfig {
            match_pattern: "git show".into(),
            transform: TransformConfig {
                add: vec!["--no-color".into()],
                ..Default::default()
            },
            optimize: OptimizeConfig::default(),
        },
        ToolConfig {
            match_pattern: "cargo build".into(),
            transform: TransformConfig {
                add: vec!["--color=never".into()],
                ..Default::default()
            },
            optimize: OptimizeConfig {
                strip: vec![
                    r"^\s*Compiling\s".into(),
                    r"^\s*Downloading\s".into(),
                ],
                keep: vec![
                    r"(?i)error".into(),
                    r"(?i)warning".into(),
                ],
                ..Default::default()
            },
        },
        ToolConfig {
            match_pattern: "cargo test".into(),
            transform: TransformConfig {
                add: vec!["--color=never".into()],
                ..Default::default()
            },
            optimize: OptimizeConfig::default(),
        },
        ToolConfig {
            match_pattern: "cargo clippy".into(),
            transform: TransformConfig {
                add: vec!["--color=never".into()],
                ..Default::default()
            },
            optimize: OptimizeConfig::default(),
        },
    ]
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
        assert!(names.contains(&"cargo build"));
        assert!(names.contains(&"cargo test"));
        assert!(names.contains(&"cargo clippy"));
    }
}
