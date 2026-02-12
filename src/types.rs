use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::tool_config::TransformConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub ref_id: String,
    pub command: String,
    pub exit_code: i32,
    pub raw_bytes: usize,
    pub optimized_bytes: usize,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub ref_id: String,
    pub command: String,
    pub exit_code: i32,
    pub raw_bytes: usize,
    pub optimized_bytes: usize,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolStats {
    pub count: u64,
    pub total_raw_bytes: u64,
    pub total_optimized_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Analytics {
    pub total_commands: u64,
    pub total_raw_bytes: u64,
    pub total_optimized_bytes: u64,
    pub total_duration_ms: u64,
    pub last_updated: Option<DateTime<Utc>>,
    pub last_cleanup: Option<DateTime<Utc>>,
    #[serde(default)]
    pub tools: HashMap<String, ToolStats>,
}

impl Analytics {
    pub fn savings_percent(&self) -> f64 {
        if self.total_raw_bytes == 0 {
            return 0.0;
        }
        let saved = self.total_raw_bytes.saturating_sub(self.total_optimized_bytes);
        (saved as f64 / self.total_raw_bytes as f64) * 100.0
    }
}

/// Commands that have meaningful subcommands worth preserving.
const SUBCOMMAND_TOOLS: &[&str] = &[
    "git", "cargo", "npm", "npx", "yarn", "pnpm", "docker", "kubectl",
    "brew", "apt", "pip", "pip3", "go", "rustup", "make", "systemctl",
    "gh", "az", "aws", "gcloud", "terraform", "helm",
];

/// Normalize a full command string into a tool name.
/// e.g. "git diff src/main.rs" -> "git diff"
///      "ls -la /some/path" -> "ls"
///      "EDITOR=vim git commit" -> "git commit"
pub fn normalize_tool(command: &str) -> String {
    let cmd = command.trim();

    // Skip leading env vars (FOO=bar) and common prefixes
    let mut parts = cmd.split_whitespace().peekable();
    while let Some(part) = parts.peek() {
        if part.contains('=') || *part == "sudo" || *part == "env" || *part == "command" {
            parts.next();
        } else {
            break;
        }
    }

    let base = match parts.next() {
        Some(b) => b,
        None => return "unknown".to_string(),
    };

    // Strip path prefix (e.g. /usr/bin/git -> git)
    let base = base.rsplit('/').next().unwrap_or(base);

    // For tools with subcommands, grab the next word that looks like a subcommand
    // (not a flag, not a path, not a quoted string)
    if SUBCOMMAND_TOOLS.contains(&base) {
        let mut skip_next = false;
        for part in parts {
            if skip_next {
                skip_next = false;
                continue;
            }
            if part.starts_with('-') {
                // Short flags like -C often take a value argument after them
                if part.len() == 2 && part.starts_with('-') {
                    skip_next = true;
                }
                continue;
            }
            // Skip things that look like paths or values, not subcommands
            if part.contains('/') || part.contains('.') || part.contains('=') {
                continue;
            }
            return format!("{base} {part}");
        }
    }

    base.to_string()
}

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct OptimizedOutput {
    pub content: String,
    pub original_bytes: usize,
    pub optimized_bytes: usize,
    pub was_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginManifest {
    pub plugins: Vec<PluginEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    pub name: String,
    pub bundle: String,
    pub source: PluginSource,
    pub installed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginSource {
    Builtin,
    Git { url: String },
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Settings {
    #[serde(flatten)]
    pub overrides: HashMap<String, PluginOverrides>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PluginOverrides {
    pub enabled: Option<bool>,
    pub transform: Option<TransformConfig>,
    pub optimize: Option<OptimizeOverrides>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct OptimizeOverrides {
    pub max_bytes: Option<usize>,
    pub strip: Option<Vec<String>>,
    pub keep: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_simple() {
        assert_eq!(normalize_tool("ls -la /some/path"), "ls");
        assert_eq!(normalize_tool("cat foo.txt"), "cat");
        assert_eq!(normalize_tool("echo hello world"), "echo");
    }

    #[test]
    fn test_normalize_subcommand() {
        assert_eq!(normalize_tool("git diff src/main.rs"), "git diff");
        assert_eq!(normalize_tool("git commit -m 'msg'"), "git commit");
        assert_eq!(normalize_tool("cargo build --release"), "cargo build");
        assert_eq!(normalize_tool("docker run -it ubuntu"), "docker run");
        assert_eq!(normalize_tool("npm install express"), "npm install");
    }

    #[test]
    fn test_normalize_env_prefix() {
        assert_eq!(normalize_tool("EDITOR=vim git commit"), "git commit");
        assert_eq!(normalize_tool("FOO=1 BAR=2 ls"), "ls");
    }

    #[test]
    fn test_normalize_sudo() {
        assert_eq!(normalize_tool("sudo git push"), "git push");
    }

    #[test]
    fn test_normalize_flags_before_subcommand() {
        assert_eq!(normalize_tool("git -C /repo diff"), "git diff");
    }

    #[test]
    fn test_normalize_path_prefix() {
        assert_eq!(normalize_tool("/usr/bin/git status"), "git status");
    }
}
