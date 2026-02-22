use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::tool_config::{TransformConfig, TruncateMode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub ref_id: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transformed_command: Option<String>,
    pub exit_code: i32,
    pub raw_bytes: usize,
    pub optimized_bytes: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_raw_bytes: Option<usize>,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub ref_id: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transformed_command: Option<String>,
    pub exit_code: i32,
    pub raw_bytes: usize,
    pub optimized_bytes: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_raw_bytes: Option<usize>,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolStats {
    pub count: u64,
    pub total_raw_bytes: u64,
    pub total_optimized_bytes: u64,
    /// Commands that exited non-zero (potential optimizer breakage signal)
    #[serde(default)]
    pub failures: u64,
    /// Times the full unoptimized log was read (signal optimizer strips too much)
    #[serde(default)]
    pub full_log_reads: u64,
    /// Times the command was actually transformed by plugin rules
    #[serde(default)]
    pub transformations: u64,
    /// Accumulated estimated raw bytes (using savings_factor when available)
    #[serde(default)]
    pub total_estimated_raw_bytes: u64,
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
    /// Accumulated estimated raw bytes (using savings_factor when available)
    #[serde(default)]
    pub total_estimated_raw_bytes: u64,
}


/// Commands that wrap/delegate to another command.
/// Each entry: (name, flags_that_consume_next_arg, skip_positional_count).
/// - flags_that_consume_next_arg: flags like `--sdk <val>` where the next word is a value
/// - skip_positional_count: number of positional (non-flag) args to skip before the real command
///   (e.g. `script` has 1 for the logfile)
const PREFIX_COMMANDS: &[(&str, &[&str], usize)] = &[
    // Simple pass-throughs (no own flags to worry about)
    ("sudo", &["-u", "-g", "-C"], 0),
    ("command", &[], 0),
    // env: skip its own flags, then continue
    ("env", &["-u", "-S"], 0),
    // Scheduling/priority wrappers
    ("nice", &["-n", "--adjustment"], 0),
    ("nohup", &[], 0),
    ("time", &["-o", "-f", "--format", "--output"], 0),
    ("caffeinate", &["-w"], 0),
    // Apple toolchain launcher
    ("xcrun", &["--sdk", "--toolchain"], 0),
    // Script captures output to a file, then runs a command
    // Pattern: script [flags] <logfile> <command...>
    ("script", &["-t", "-T"], 1),
    // direnv exec <dir> <command...>
    ("direnv", &[], 2),
    // timeout [flags] <duration> <command...>
    ("timeout", &["--signal", "-s", "--kill-after", "-k"], 1),
];

/// Normalize a full command string into a tool name.
/// e.g. "git diff src/main.rs" -> "git diff"
///      "ls -la /some/path" -> "ls"
///      "EDITOR=vim git commit" -> "git commit"
///      "script -q /tmp/log xcodebuild test" -> "xcodebuild test"
///      "xcrun --sdk iphoneos xcodebuild build" -> "xcodebuild build"
///
/// When `patterns` is provided, uses longest-prefix matching against registered
/// plugin match patterns for multi-level subcommand detection (e.g. "xcresulttool get test-results").
/// When no pattern matches, falls back to base + first non-flag non-path word.
pub fn normalize_tool(command: &str, patterns: &HashSet<String>) -> String {
    let words: Vec<&str> = command.trim().split_whitespace().collect();
    let mut i = 0;

    // Phase 1: Skip env vars and prefix/wrapper commands
    'outer: loop {
        if i >= words.len() {
            return "unknown".to_string();
        }

        let word = words[i].rsplit('/').next().unwrap_or(words[i]);

        // Skip env-var assignments (FOO=bar)
        if word.contains('=') {
            i += 1;
            continue;
        }

        // Check prefix commands table
        if let Some(&(_, value_flags, skip_positional)) =
            PREFIX_COMMANDS.iter().find(|&&(name, _, _)| name == word)
        {
            i += 1;
            // Skip the prefix's own flags
            while i < words.len() && words[i].starts_with('-') {
                if value_flags.contains(&words[i]) {
                    i += 2; // flag + its value
                } else {
                    i += 1; // boolean flag
                }
            }
            // Skip positional args belonging to the prefix (e.g. script's logfile)
            for _ in 0..skip_positional {
                if i < words.len() {
                    i += 1;
                }
            }
            continue 'outer;
        }

        // Not a prefix/wrapper — this is the real command
        break;
    }

    if i >= words.len() {
        return "unknown".to_string();
    }

    // Phase 2: Extract base command (strip path prefix)
    let base = words[i].rsplit('/').next().unwrap_or(words[i]);
    i += 1;

    // Phase 3: Collect candidate subcommand tokens (skip flags and paths)
    let mut sub_tokens: Vec<&str> = Vec::new();
    let mut skip_next = false;
    for &part in &words[i..] {
        if skip_next {
            skip_next = false;
            continue;
        }
        if part.starts_with('-') {
            if part.len() == 2 {
                skip_next = true;
            }
            continue;
        }
        if part.contains('/') || part.contains('.') || part.contains('=') {
            continue;
        }
        sub_tokens.push(part);
    }

    // Phase 4: Longest-prefix match against registered patterns
    if !patterns.is_empty() {
        for len in (1..=sub_tokens.len()).rev() {
            let candidate = std::iter::once(base)
                .chain(sub_tokens[..len].iter().copied())
                .collect::<Vec<_>>()
                .join(" ");
            if patterns.contains(&candidate) {
                return candidate;
            }
        }
        // Check if just the base matches a pattern
        if patterns.contains(base) {
            return base.to_string();
        }
    }

    // Phase 5: Fallback — base + first subcommand token (preserves behavior for unregistered tools)
    if let Some(&first_sub) = sub_tokens.first() {
        return format!("{base} {first_sub}");
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
#[allow(dead_code)]
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
    pub max_lines: Option<usize>,
    pub strip: Option<Vec<String>>,
    pub keep: Option<Vec<String>>,
    pub replace: Option<Vec<crate::tool_config::ReplaceRule>>,
    pub truncate: Option<TruncateMode>,
    pub optimize_stderr: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: builds a pattern set from a list of match strings.
    fn patterns(pats: &[&str]) -> HashSet<String> {
        pats.iter().map(|s| s.to_string()).collect()
    }

    /// Patterns matching the builtin plugins.
    fn default_patterns() -> HashSet<String> {
        patterns(&[
            "git diff", "git log", "git status", "git show", "git blame",
            "git branch", "git stash",
            "cargo build", "cargo test", "cargo clippy",
            "gh issue", "gh pr", "gh repo", "gh run", "gh workflow", "gh api",
        ])
    }

    #[test]
    fn test_normalize_simple() {
        let p = default_patterns();
        assert_eq!(normalize_tool("ls -la /some/path", &p), "ls");
        assert_eq!(normalize_tool("cat foo.txt", &p), "cat");
        assert_eq!(normalize_tool("echo hello world", &p), "echo hello");
    }

    #[test]
    fn test_normalize_subcommand() {
        let p = default_patterns();
        assert_eq!(normalize_tool("git diff src/main.rs", &p), "git diff");
        assert_eq!(normalize_tool("git commit -m 'msg'", &p), "git commit");
        assert_eq!(normalize_tool("cargo build --release", &p), "cargo build");
        assert_eq!(normalize_tool("docker run -it ubuntu", &p), "docker run");
        assert_eq!(normalize_tool("npm install express", &p), "npm install");
    }

    #[test]
    fn test_normalize_env_prefix() {
        let p = default_patterns();
        assert_eq!(normalize_tool("EDITOR=vim git commit", &p), "git commit");
        assert_eq!(normalize_tool("FOO=1 BAR=2 ls", &p), "ls");
    }

    #[test]
    fn test_normalize_sudo() {
        let p = default_patterns();
        assert_eq!(normalize_tool("sudo git push", &p), "git push");
    }

    #[test]
    fn test_normalize_flags_before_subcommand() {
        let p = default_patterns();
        assert_eq!(normalize_tool("git -C /repo diff", &p), "git diff");
    }

    #[test]
    fn test_normalize_path_prefix() {
        let p = default_patterns();
        assert_eq!(normalize_tool("/usr/bin/git status", &p), "git status");
    }

    #[test]
    fn test_normalize_transparent_prefixes() {
        let p = default_patterns();
        assert_eq!(normalize_tool("nice -n 10 cargo build", &p), "cargo build");
        assert_eq!(normalize_tool("nohup python3 server.py", &p), "python3");
        assert_eq!(normalize_tool("time cargo test", &p), "cargo test");
        assert_eq!(normalize_tool("caffeinate cargo build --release", &p), "cargo build");
    }

    #[test]
    fn test_normalize_script_wrapper() {
        let p = default_patterns();
        assert_eq!(
            normalize_tool("script -q /tmp/st9.log xcodebuild test -scheme Foo", &p),
            "xcodebuild test"
        );
        assert_eq!(
            normalize_tool("script /tmp/log ls -la", &p),
            "ls"
        );
        assert_eq!(
            normalize_tool("script -q -a /tmp/log git diff", &p),
            "git diff"
        );
    }

    #[test]
    fn test_normalize_xcrun_wrapper() {
        let p = patterns(&["xcresulttool get test-results", "xcresulttool get", "xcresulttool export", "xcodebuild build"]);
        assert_eq!(
            normalize_tool("xcrun xcresulttool get test-results summary --path /foo", &p),
            "xcresulttool get test-results"
        );
        assert_eq!(
            normalize_tool("xcrun --sdk iphoneos xcodebuild build", &p),
            "xcodebuild build"
        );
        assert_eq!(
            normalize_tool("xcrun -v xcresulttool export", &p),
            "xcresulttool export"
        );
    }

    #[test]
    fn test_normalize_nested_wrappers() {
        let p = patterns(&["xcresulttool get", "cargo build"]);
        assert_eq!(
            normalize_tool("script -q /tmp/log xcrun xcresulttool get", &p),
            "xcresulttool get"
        );
        assert_eq!(
            normalize_tool("sudo nice -n 5 cargo build", &p),
            "cargo build"
        );
    }

    #[test]
    fn test_normalize_apple_toolchain() {
        let p = patterns(&["xcodebuild test", "swift build", "swift package", "pod install", "flutter test"]);
        assert_eq!(
            normalize_tool("xcodebuild test -scheme PentaPrism", &p),
            "xcodebuild test"
        );
        assert_eq!(normalize_tool("swift build", &p), "swift build");
        assert_eq!(normalize_tool("swift package resolve", &p), "swift package");
        assert_eq!(normalize_tool("pod install", &p), "pod install");
        assert_eq!(normalize_tool("flutter test --coverage", &p), "flutter test");
    }

    #[test]
    fn test_normalize_build_tools() {
        let p = patterns(&["bazel build", "bazel test"]);
        assert_eq!(normalize_tool("bazel build //src:target", &p), "bazel build");
        assert_eq!(normalize_tool("bazel test //tests:all", &p), "bazel test");
    }

    #[test]
    fn test_normalize_deep_subcommand() {
        // Multi-level: "xcresulttool get test-results" matches deeper than "xcresulttool get"
        let p = patterns(&["xcresulttool get", "xcresulttool get test-results"]);
        assert_eq!(
            normalize_tool("xcresulttool get test-results summary --path /foo", &p),
            "xcresulttool get test-results"
        );
        // Falls back to shorter match when deeper doesn't exist
        assert_eq!(
            normalize_tool("xcresulttool get export-data --verbose", &p),
            "xcresulttool get"
        );
    }

    #[test]
    fn test_normalize_unknown_fallback() {
        // Unknown tool with no matching pattern: base + first subcommand token
        let p = default_patterns();
        assert_eq!(normalize_tool("terraform plan --var-file=prod", &p), "terraform plan");
        assert_eq!(normalize_tool("kubectl get pods", &p), "kubectl get");
        assert_eq!(normalize_tool("aws s3 cp file s3://bucket", &p), "aws s3");
    }

    #[test]
    fn test_normalize_empty_patterns() {
        // With empty patterns, still gets base + first subcommand fallback
        let p = HashSet::new();
        assert_eq!(normalize_tool("git diff src/main.rs", &p), "git diff");
        assert_eq!(normalize_tool("ls -la", &p), "ls");
    }

    #[test]
    fn test_normalize_direnv_wrapper() {
        let p = default_patterns();
        assert_eq!(
            normalize_tool("direnv exec /project git diff", &p),
            "git diff"
        );
        assert_eq!(
            normalize_tool("direnv exec . cargo build", &p),
            "cargo build"
        );
    }

    #[test]
    fn test_normalize_timeout_wrapper() {
        let p = default_patterns();
        assert_eq!(
            normalize_tool("timeout 30 cargo test", &p),
            "cargo test"
        );
        assert_eq!(
            normalize_tool("timeout --signal KILL 60 git status", &p),
            "git status"
        );
        assert_eq!(
            normalize_tool("timeout -k 10 30 cargo build", &p),
            "cargo build"
        );
    }
}
