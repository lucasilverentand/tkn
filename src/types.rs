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
    /// Commands that exited non-zero (potential optimizer breakage signal)
    #[serde(default)]
    pub failures: u64,
    /// Times the full unoptimized log was read (signal optimizer strips too much)
    #[serde(default)]
    pub full_log_reads: u64,
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
const SUBCOMMAND_TOOLS: &[(&str, &[&str])] = &[
    ("git", &["add", "bisect", "blame", "branch", "checkout", "cherry-pick", "clone",
              "commit", "config", "diff", "fetch", "init", "log", "merge", "mv",
              "pull", "push", "rebase", "reflog", "remote", "reset", "restore",
              "revert", "rm", "show", "stash", "status", "submodule", "switch", "tag",
              "worktree"]),
    ("cargo", &["build", "test", "run", "check", "clippy", "fmt", "bench", "doc",
                "publish", "install", "update", "add", "remove", "init", "new", "clean"]),
    ("npm", &["install", "run", "test", "build", "publish", "init", "update", "ci",
              "exec", "start", "pack", "link", "uninstall", "audit"]),
    ("npx", &[]),
    ("yarn", &["add", "remove", "install", "run", "build", "test", "init", "publish"]),
    ("pnpm", &["add", "remove", "install", "run", "build", "test", "init", "publish",
               "exec", "dlx"]),
    ("docker", &["build", "run", "push", "pull", "exec", "compose", "stop", "start",
                 "logs", "ps", "images", "volume", "network", "system"]),
    ("kubectl", &["get", "apply", "delete", "describe", "logs", "exec", "create",
                  "edit", "scale", "rollout", "port-forward", "config"]),
    ("brew", &["install", "uninstall", "update", "upgrade", "search", "info",
               "list", "tap", "cask", "services"]),
    ("apt", &["install", "remove", "update", "upgrade", "search", "list", "purge"]),
    ("pip", &["install", "uninstall", "freeze", "list", "show", "search"]),
    ("pip3", &["install", "uninstall", "freeze", "list", "show", "search"]),
    ("go", &["build", "test", "run", "get", "mod", "fmt", "vet", "install", "generate"]),
    ("rustup", &["update", "default", "target", "component", "toolchain", "override"]),
    ("make", &[]),
    ("systemctl", &["start", "stop", "restart", "status", "enable", "disable",
                    "reload", "daemon-reload"]),
    ("gh", &["pr", "issue", "repo", "auth", "api", "run", "release", "gist",
             "codespace", "workflow"]),
    ("az", &["login", "group", "vm", "storage", "webapp", "acr", "aks"]),
    ("aws", &["s3", "ec2", "iam", "lambda", "ecs", "cloudformation", "sts",
              "ssm", "logs"]),
    ("gcloud", &["auth", "config", "compute", "container", "functions", "run",
                 "app", "builds"]),
    ("terraform", &["init", "plan", "apply", "destroy", "validate", "fmt",
                    "import", "state", "output"]),
    ("helm", &["install", "upgrade", "uninstall", "repo", "list", "rollback",
               "template", "lint", "package"]),
    // Apple/Xcode toolchain
    ("xcodebuild", &["build", "test", "clean", "archive", "analyze",
                     "build-for-testing", "test-without-building"]),
    ("xcresulttool", &["get", "export", "merge", "format"]),
    ("swift", &["build", "test", "run", "package"]),
    ("xctrace", &["record", "export", "list"]),
    ("swiftlint", &["lint", "analyze", "rules"]),
    // Package managers / build tools
    ("pod", &["install", "update", "init", "repo", "spec", "lib"]),
    ("flutter", &["build", "test", "run", "analyze", "pub", "create", "doctor"]),
    ("gradlew", &["build", "test", "assemble", "clean", "check"]),
    ("bazel", &["build", "test", "run", "query", "clean", "fetch", "info"]),
];

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
];

/// Normalize a full command string into a tool name.
/// e.g. "git diff src/main.rs" -> "git diff"
///      "ls -la /some/path" -> "ls"
///      "EDITOR=vim git commit" -> "git commit"
///      "script -q /tmp/log xcodebuild test" -> "xcodebuild test"
///      "xcrun --sdk iphoneos xcodebuild build" -> "xcodebuild build"
pub fn normalize_tool(command: &str) -> String {
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

    // Phase 3: For subcommand tools, find the subcommand
    if let Some(&(_, known_subs)) = SUBCOMMAND_TOOLS.iter().find(|&&(name, _)| name == base) {
        let mut skip_next = false;
        for &part in &words[i..] {
            if skip_next {
                skip_next = false;
                continue;
            }
            if part.starts_with('-') {
                // Short flags like -C often take a value argument
                if part.len() == 2 {
                    skip_next = true;
                }
                continue;
            }
            // Skip things that look like paths or values, not subcommands
            if part.contains('/') || part.contains('.') || part.contains('=') {
                continue;
            }
            // If we have a known subcommand list, validate against it
            if !known_subs.is_empty() && !known_subs.contains(&part) {
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

    #[test]
    fn test_normalize_transparent_prefixes() {
        assert_eq!(normalize_tool("nice -n 10 cargo build"), "cargo build");
        assert_eq!(normalize_tool("nohup python3 server.py"), "python3");
        assert_eq!(normalize_tool("time cargo test"), "cargo test");
        assert_eq!(normalize_tool("caffeinate cargo build --release"), "cargo build");
    }

    #[test]
    fn test_normalize_script_wrapper() {
        assert_eq!(
            normalize_tool("script -q /tmp/st9.log xcodebuild test -scheme Foo"),
            "xcodebuild test"
        );
        assert_eq!(
            normalize_tool("script /tmp/log ls -la"),
            "ls"
        );
        assert_eq!(
            normalize_tool("script -q -a /tmp/log git diff"),
            "git diff"
        );
    }

    #[test]
    fn test_normalize_xcrun_wrapper() {
        assert_eq!(
            normalize_tool("xcrun xcresulttool get test-results summary --path /foo"),
            "xcresulttool get"
        );
        assert_eq!(
            normalize_tool("xcrun --sdk iphoneos xcodebuild build"),
            "xcodebuild build"
        );
        assert_eq!(
            normalize_tool("xcrun -v xcresulttool export"),
            "xcresulttool export"
        );
    }

    #[test]
    fn test_normalize_nested_wrappers() {
        assert_eq!(
            normalize_tool("script -q /tmp/log xcrun xcresulttool get"),
            "xcresulttool get"
        );
        assert_eq!(
            normalize_tool("sudo nice -n 5 cargo build"),
            "cargo build"
        );
    }

    #[test]
    fn test_normalize_apple_toolchain() {
        assert_eq!(
            normalize_tool("xcodebuild test -scheme PentaPrism"),
            "xcodebuild test"
        );
        assert_eq!(normalize_tool("swift build"), "swift build");
        assert_eq!(normalize_tool("swift package resolve"), "swift package");
        assert_eq!(normalize_tool("pod install"), "pod install");
        assert_eq!(normalize_tool("flutter test --coverage"), "flutter test");
    }

    #[test]
    fn test_normalize_build_tools() {
        assert_eq!(normalize_tool("bazel build //src:target"), "bazel build");
        assert_eq!(normalize_tool("bazel test //tests:all"), "bazel test");
    }
}
