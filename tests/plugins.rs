//! Integration tests for all built-in plugins.
//!
//! These tests validate that every plugin TOML file:
//! - Parses correctly
//! - Has valid regex patterns (strip, keep, replace)
//! - Has reasonable configuration values
//!
//! And that the optimizer pipeline produces expected results
//! when fed realistic sample output for each tool category.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use regex::Regex;
use serde::Deserialize;

// ── Minimal plugin types (mirrors src/tool_config.rs) ──────────────────────

#[derive(Debug, Deserialize)]
struct ToolConfig {
    #[serde(rename = "match")]
    match_pattern: String,
    #[serde(default)]
    transform: TransformConfig,
    #[serde(default)]
    optimize: OptimizeConfig,
}

#[derive(Debug, Deserialize, Default)]
struct TransformConfig {
    #[serde(default)]
    add: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    remove: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    replace: HashMap<String, String>,
    #[allow(dead_code)]
    savings_factor: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
struct OptimizeConfig {
    #[serde(default)]
    strip: Vec<String>,
    #[serde(default)]
    keep: Vec<String>,
    #[serde(default)]
    replace: Vec<ReplaceRule>,
    max_lines: Option<usize>,
    #[serde(default)]
    #[allow(dead_code)]
    raw: bool,
    #[serde(default)]
    truncate: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    optimize_stderr: bool,
    #[serde(default)]
    #[allow(dead_code)]
    compact_json: bool,
}

#[derive(Debug, Deserialize)]
struct ReplaceRule {
    pattern: String,
    #[serde(default)]
    replacement: String,
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn load_all_plugins() -> Vec<(String, ToolConfig)> {
    let plugins_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let mut plugins = Vec::new();

    for bundle_entry in fs::read_dir(&plugins_dir).expect("plugins/ dir missing") {
        let bundle = bundle_entry.unwrap();
        if !bundle.file_type().unwrap().is_dir() {
            continue;
        }
        for file_entry in fs::read_dir(bundle.path()).unwrap() {
            let file = file_entry.unwrap();
            let path = file.path();
            if path.extension().is_some_and(|e| e == "toml") {
                let name = format!(
                    "{}/{}",
                    bundle.file_name().to_string_lossy(),
                    path.file_stem().unwrap().to_string_lossy()
                );
                let content = fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("failed to read {name}: {e}"));
                let config: ToolConfig = toml::from_str(&content)
                    .unwrap_or_else(|e| panic!("failed to parse {name}: {e}"));
                plugins.push((name, config));
            }
        }
    }

    assert!(!plugins.is_empty(), "no plugins found");
    plugins
}

fn compile_regex(pattern: &str) -> Result<Regex, regex::Error> {
    Regex::new(pattern)
}

/// Apply strip/keep/replace filters (mirrors optimizer logic).
fn apply_filters(input: &str, config: &OptimizeConfig) -> String {
    let has_keep = !config.keep.is_empty();
    let has_strip = !config.strip.is_empty();

    let keep_regexes: Vec<Regex> = config.keep.iter().map(|p| Regex::new(p).unwrap()).collect();
    let strip_regexes: Vec<Regex> = config
        .strip
        .iter()
        .map(|p| Regex::new(p).unwrap())
        .collect();
    let replace_regexes: Vec<(Regex, &str)> = config
        .replace
        .iter()
        .map(|r| (Regex::new(&r.pattern).unwrap(), r.replacement.as_str()))
        .collect();

    let mut lines: Vec<String> = Vec::new();

    for line in input.lines() {
        // keep filter takes priority
        if has_keep {
            let kept = keep_regexes.iter().any(|r| r.is_match(line));
            if !kept {
                continue;
            }
        } else if has_strip {
            let stripped = strip_regexes.iter().any(|r| r.is_match(line));
            if stripped {
                continue;
            }
        }

        let mut l = line.to_string();
        for (re, rep) in &replace_regexes {
            l = re.replace_all(&l, *rep).to_string();
        }
        lines.push(l);
    }

    lines.join("\n")
}

// ── Generic validation tests ───────────────────────────────────────────────

#[test]
fn all_plugins_parse_successfully() {
    let plugins = load_all_plugins();
    // Just loading them validates parsing — the helper panics on failure.
    assert!(
        plugins.len() >= 80,
        "expected at least 80 plugins, got {}",
        plugins.len()
    );
}

#[test]
fn all_plugins_have_match_pattern() {
    for (name, config) in load_all_plugins() {
        assert!(
            !config.match_pattern.is_empty(),
            "plugin {name} has empty match pattern"
        );
    }
}

#[test]
fn all_strip_regexes_compile() {
    for (name, config) in load_all_plugins() {
        for (i, pattern) in config.optimize.strip.iter().enumerate() {
            compile_regex(pattern).unwrap_or_else(|e| {
                panic!("plugin {name} strip[{i}] regex invalid: {pattern:?} — {e}")
            });
        }
    }
}

#[test]
fn all_keep_regexes_compile() {
    for (name, config) in load_all_plugins() {
        for (i, pattern) in config.optimize.keep.iter().enumerate() {
            compile_regex(pattern).unwrap_or_else(|e| {
                panic!("plugin {name} keep[{i}] regex invalid: {pattern:?} — {e}")
            });
        }
    }
}

#[test]
fn all_replace_regexes_compile() {
    for (name, config) in load_all_plugins() {
        for (i, rule) in config.optimize.replace.iter().enumerate() {
            compile_regex(&rule.pattern).unwrap_or_else(|e| {
                panic!(
                    "plugin {name} replace[{i}] regex invalid: {:?} — {e}",
                    rule.pattern
                )
            });
        }
    }
}

#[test]
fn all_transform_add_flags_are_valid() {
    for (name, config) in load_all_plugins() {
        for flag in &config.transform.add {
            let variants: Vec<&str> = flag.split('|').collect();
            let canonical = variants[0];
            assert!(
                canonical.starts_with('-'),
                "plugin {name} transform.add canonical {canonical:?} doesn't start with '-'"
            );
        }
    }
}

#[test]
fn max_lines_are_reasonable() {
    for (name, config) in load_all_plugins() {
        if let Some(max) = config.optimize.max_lines {
            assert!(
                max > 0 && max <= 10000,
                "plugin {name} max_lines={max} seems unreasonable"
            );
        }
    }
}

#[test]
fn truncate_modes_are_valid() {
    for (name, config) in load_all_plugins() {
        if let Some(ref mode) = config.optimize.truncate {
            assert!(
                mode == "top" || mode == "middle" || mode == "bottom",
                "plugin {name} truncate={mode:?} is not a valid mode"
            );
        }
    }
}

#[test]
fn no_duplicate_match_patterns() {
    let plugins = load_all_plugins();
    let mut seen = HashMap::new();
    for (name, config) in &plugins {
        if let Some(prev) = seen.insert(&config.match_pattern, name) {
            panic!(
                "duplicate match pattern {:?}: {prev} and {name}",
                config.match_pattern
            );
        }
    }
}

// ── Git plugin tests ───────────────────────────────────────────────────────

#[test]
fn git_diff_strips_metadata_keeps_hunks() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "git/diff").unwrap();

    let input = "\
diff --git a/src/main.rs b/src/main.rs
index abc1234..def5678 100644
old mode 100644
new mode 100755
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,6 +10,7 @@ fn main() {
     let x = 1;
+    let y = 2;
     println!(\"hello\");
\\ No newline at end of file
Binary files a/img.png and b/img.png differ";

    let result = apply_filters(input, &config.optimize);
    assert!(!result.contains("diff --git"), "should strip diff header");
    assert!(!result.contains("index abc1234"), "should strip index line");
    assert!(!result.contains("old mode"), "should strip mode lines");
    assert!(!result.contains("--- a/"), "should strip --- line");
    assert!(!result.contains("+++ b/"), "should strip +++ line");
    assert!(
        !result.contains("No newline"),
        "should strip no-newline marker"
    );
    assert!(
        !result.contains("Binary files"),
        "should strip binary notice"
    );
    assert!(result.contains("+    let y = 2;"), "should keep additions");
    assert!(result.contains("let x = 1;"), "should keep context");
}

#[test]
fn git_diff_replace_strips_hunk_context() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "git/diff").unwrap();
    let replace = &config.optimize.replace;
    assert!(!replace.is_empty(), "git diff should have replace rules");

    let re = Regex::new(&replace[0].pattern).unwrap();
    let line = "@@ -10,6 +10,7 @@ fn main() {";
    let result = re.replace(line, &replace[0].replacement);
    assert_eq!(result, "@@ -10,6 +10,7 @@");
}

#[test]
fn git_diff_transform_adds_no_color() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "git/diff").unwrap();
    assert!(
        config
            .transform
            .add
            .iter()
            .any(|f| f.contains("--no-color")),
        "git diff should add --no-color"
    );
}

#[test]
fn git_status_transform_adds_short() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "git/status").unwrap();
    assert!(
        config.transform.add.iter().any(|f| f.contains("--short")),
        "git status should add --short"
    );
}

#[test]
fn git_status_strips_hints_and_os_artifacts() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "git/status").unwrap();

    let input = "\
 M src/main.rs
?? new_file.rs
   (use \"git add\" to stage)
.DS_Store
Thumbs.db
   (all conflicts fixed";

    let result = apply_filters(input, &config.optimize);
    assert!(
        result.contains("M src/main.rs"),
        "should keep modified files"
    );
    assert!(
        result.contains("new_file.rs"),
        "should keep untracked files"
    );
    assert!(
        !result.contains("use \"git add\""),
        "should strip git hints"
    );
    assert!(!result.contains(".DS_Store"), "should strip .DS_Store");
    assert!(!result.contains("Thumbs.db"), "should strip Thumbs.db");
}

#[test]
fn git_log_transform_adds_oneline_and_limit() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "git/log").unwrap();
    assert!(
        config.transform.add.iter().any(|f| f.contains("--oneline")),
        "git log should add --oneline"
    );
    assert!(
        config.transform.add.iter().any(|f| f.contains("-20")),
        "git log should limit to 20"
    );
}

#[test]
fn git_log_strips_verbose_metadata() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "git/log").unwrap();

    let input = "\
commit abc1234def5678901234567890abcdef12345678
Author: Luca <luca@example.com>
Date:   Mon Mar 24 10:00:00 2026
Merge: abc1234 def5678

    feat: add new feature

gpg: Signature made Mon Mar 24
Primary key fingerprint: ABCD 1234";

    let result = apply_filters(input, &config.optimize);
    assert!(!result.contains("Author:"), "should strip Author");
    assert!(!result.contains("Date:"), "should strip Date");
    assert!(!result.contains("Merge:"), "should strip Merge");
    assert!(!result.contains("gpg:"), "should strip gpg");
    assert!(!result.contains("Primary key"), "should strip fingerprint");
    assert!(
        result.contains("feat: add new feature"),
        "should keep message"
    );
}

#[test]
fn git_log_replace_shortens_hashes() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "git/log").unwrap();
    let replace = &config.optimize.replace;
    assert!(!replace.is_empty());

    let re = Regex::new(&replace[0].pattern).unwrap();
    let line = "abc1234def5678901234567890abcdef12345678 feat: something";
    let result = re.replace(line, &replace[0].replacement);
    assert_eq!(result, "abc1234 feat: something");
}

#[test]
fn git_commit_strips_file_change_details() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "git/commit").unwrap();

    let input = "\
[main abc1234] feat: add feature
 create mode 100644 src/new.rs
 delete mode 100644 src/old.rs
 rename src/{old.rs => new.rs}
 src/main.rs | 5 +++--
 2 files changed, 3 insertions(+), 2 deletions(-)";

    let result = apply_filters(input, &config.optimize);
    assert!(
        result.contains("feat: add feature"),
        "should keep commit msg"
    );
    assert!(!result.contains("create mode"), "should strip create mode");
    assert!(!result.contains("delete mode"), "should strip delete mode");
    assert!(!result.contains("rename src"), "should strip rename");
}

#[test]
fn git_show_strips_diff_and_commit_metadata() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "git/show").unwrap();

    let input = "\
commit abc1234def5678901234567890abcdef12345678
Author: Luca <luca@example.com>
Date:   Mon Mar 24 10:00:00 2026

    feat: something

diff --git a/src/main.rs b/src/main.rs
index abc1234..def5678 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@ fn main() {
+    let x = 1;";

    let result = apply_filters(input, &config.optimize);
    assert!(!result.contains("Author:"));
    assert!(!result.contains("diff --git"));
    assert!(!result.contains("index abc"));
    assert!(result.contains("feat: something"));
    assert!(result.contains("+    let x = 1;"));
}

// ── Cargo plugin tests ─────────────────────────────────────────────────────

#[test]
fn cargo_build_keeps_errors_strips_progress() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "cargo/build").unwrap();

    let input = "\
   Compiling serde v1.0.0
   Compiling tkn v0.1.0
   Checking regex v1.0.0
   Downloading crates ...
   Downloaded serde v1.0.0
   Finished dev target(s) in 5.32s
   Fresh serde v1.0.0
   Locking 5 packages
error[E0308]: mismatched types
 --> src/main.rs:10:5
warning: unused variable `x`
 --> src/main.rs:5:9";

    let result = apply_filters(input, &config.optimize);
    assert!(!result.contains("Compiling"), "should strip Compiling");
    assert!(!result.contains("Checking"), "should strip Checking");
    assert!(!result.contains("Downloading"), "should strip Downloading");
    assert!(!result.contains("Finished"), "should strip Finished");
    assert!(!result.contains("Fresh"), "should strip Fresh");
    assert!(!result.contains("Locking"), "should strip Locking");
    assert!(result.contains("error[E0308]"), "should keep errors");
    assert!(result.contains("warning: unused"), "should keep warnings");
}

#[test]
fn cargo_test_strips_passing_keeps_failures() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "cargo/test").unwrap();

    let input = "\
   Compiling tkn v0.1.0
   Finished test target(s) in 3.21s
     Running unittests src/main.rs
running 5 tests
test tests::test_one ... ok
test tests::test_two ... ok
test tests::test_three ... FAILED
test result: FAILED. 2 passed; 1 failed;

---- tests::test_three stdout ----
thread 'tests::test_three' panicked at 'assertion failed'";

    let result = apply_filters(input, &config.optimize);
    assert!(!result.contains("Compiling"), "should strip Compiling");
    assert!(!result.contains("... ok"), "should strip passing tests");
    assert!(!result.contains("running 5"), "should strip running count");
    assert!(
        result.contains("test_three ... FAILED"),
        "should keep failures"
    );
    assert!(result.contains("panicked at"), "should keep panic output");
}

#[test]
fn cargo_clippy_strips_build_noise() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "cargo/clippy").unwrap();

    let input = "\
    Checking tkn v0.1.0
    Compiling serde v1.0.0
    Finished dev target(s) in 2.5s
warning: redundant clone
  --> src/main.rs:5:10
   |
5  |     x.clone()
   |      ^^^^^^^^";

    let result = apply_filters(input, &config.optimize);
    assert!(!result.contains("Checking"), "should strip Checking");
    assert!(!result.contains("Compiling"), "should strip Compiling");
    assert!(!result.contains("Finished"), "should strip Finished");
    assert!(
        result.contains("warning: redundant clone"),
        "should keep warnings"
    );
}

// ── Node/JS plugin tests ───────────────────────────────────────────────────

#[test]
fn npm_run_strips_lifecycle_headers() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "npm/run").unwrap();

    let input = "\
> myapp@1.0.0 build
> tsc && vite build

npm warn deprecated package
npm notice some notice
src/main.ts(5,3): error TS2304: Cannot find name 'foo'.";

    let result = apply_filters(input, &config.optimize);
    assert!(
        !result.contains("> myapp@1.0.0"),
        "should strip lifecycle header"
    );
    assert!(!result.contains("npm warn"), "should strip npm warn");
    assert!(!result.contains("npm notice"), "should strip npm notice");
    assert!(result.contains("error TS2304"), "should keep actual errors");
}

#[test]
fn tsc_strips_noise_keeps_errors() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "tsc/tsc").unwrap();

    let input = "\
src/main.ts(5,3): error TS2304: Cannot find name 'foo'.
  Overload 1 of 3, '(x: string): void'
  Overload 2 of 3, '(x: number): void'
The following changes are being made to your tsconfig.json
  The expected type comes from property 'x'";

    let result = apply_filters(input, &config.optimize);
    assert!(result.contains("error TS2304"), "should keep errors");
    assert!(
        !result.contains("Overload 1"),
        "should strip overload noise"
    );
    assert!(
        !result.contains("following changes"),
        "should strip tsconfig notice"
    );
    assert!(
        !result.contains("expected type comes"),
        "should strip type hint"
    );
}

#[test]
fn bun_test_strips_passing_details() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "bun/test").unwrap();

    let input = "\
(pass) test one
(pass) test two
(fail) test three
  Expected: 1
  Received: 2
test result: 1 failed, 2 passed";

    let result = apply_filters(input, &config.optimize);
    assert!(!result.contains("(pass)"), "should strip passing tests");
    assert!(result.contains("(fail) test three"), "should keep failures");
    assert!(result.contains("test result"), "should keep summary");
}

// ── Docker plugin tests ────────────────────────────────────────────────────

#[test]
fn docker_build_keeps_errors_strips_internals() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "docker/build").unwrap();

    let input = "\
#1 [internal] load build definition
#2 sha256:abc123 0.1s
#3 DONE 0.5s
#4 CACHED
#5 extracting sha256:def456
#6 transferring context: 2.1kB
#7 resolve docker.io/library/node:18
#8 ERROR: failed to build
error: COPY failed: file not found
warning: deprecated Dockerfile instruction
exporting to image
naming to docker.io/library/myapp:latest";

    let result = apply_filters(input, &config.optimize);
    assert!(
        !result.contains("[internal]"),
        "should strip internal steps"
    );
    assert!(!result.contains("sha256:"), "should strip sha256 refs");
    assert!(!result.contains("DONE 0.5s"), "should strip DONE");
    assert!(!result.contains("CACHED"), "should strip CACHED");
    assert!(result.contains("ERROR: failed"), "should keep errors");
    assert!(
        result.contains("warning: deprecated"),
        "should keep warnings"
    );
    assert!(
        result.contains("exporting to image"),
        "should keep export info"
    );
    assert!(result.contains("naming to"), "should keep naming info");
}

#[test]
fn docker_ps_has_reasonable_limits() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "docker/ps").unwrap();
    assert_eq!(config.optimize.max_lines, Some(50));
    assert_eq!(config.optimize.truncate.as_deref(), Some("top"));
}

// ── Python plugin tests ────────────────────────────────────────────────────

#[test]
fn pytest_transform_adds_short_tb_and_quiet() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "pytest/pytest").unwrap();
    assert!(
        config
            .transform
            .add
            .iter()
            .any(|f| f.contains("--tb=short")),
        "pytest should add --tb=short"
    );
    assert!(
        config.transform.add.iter().any(|f| f.contains("-q")),
        "pytest should add -q"
    );
}

#[test]
fn pytest_strips_session_header() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "pytest/pytest").unwrap();

    let input = "\
========================= test session starts =========================
platform linux -- Python 3.12.0, pytest-8.0.0
rootdir: /home/user/project
configfile: pyproject.toml
plugins: cov-4.1.0
collected 42 items

PASSED test_foo.py::test_one
FAILED test_foo.py::test_two
E       assert 1 == 2
FAILED test_foo.py::test_three
short test summary info
1 failed, 1 passed";

    let result = apply_filters(input, &config.optimize);
    assert!(
        !result.contains("test session starts"),
        "should strip session header"
    );
    assert!(!result.contains("platform linux"), "should strip platform");
    assert!(!result.contains("rootdir:"), "should strip rootdir");
    assert!(!result.contains("configfile:"), "should strip configfile");
    assert!(!result.contains("plugins:"), "should strip plugins");
    assert!(!result.contains("collected 42"), "should strip collected");
    assert!(!result.contains("PASSED"), "should strip PASSED lines");
    assert!(result.contains("FAILED"), "should keep FAILED");
    assert!(
        result.contains("assert 1 == 2"),
        "should keep assertion details"
    );
}

#[test]
fn ruff_check_has_reasonable_config() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "ruff/check").unwrap();
    assert!(!config.match_pattern.is_empty());
    assert!(config.optimize.max_lines.is_some());
}

// ── Go plugin tests ────────────────────────────────────────────────────────

#[test]
fn go_test_strips_passing_keeps_failures() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "go/test").unwrap();

    let input = "\
=== RUN   TestAdd
=== PAUSE TestAdd
=== CONT  TestAdd
--- PASS: TestAdd (0.00s)
ok      example.com/pkg  0.123s
=== RUN   TestSub
--- FAIL: TestSub (0.01s)
    sub_test.go:10: expected 3, got 2
FAIL    example.com/pkg  0.456s";

    let result = apply_filters(input, &config.optimize);
    assert!(!result.contains("=== RUN"), "should strip RUN");
    assert!(!result.contains("=== PAUSE"), "should strip PAUSE");
    assert!(!result.contains("=== CONT"), "should strip CONT");
    assert!(!result.contains("--- PASS"), "should strip PASS");
    assert!(
        !result.contains("ok      example"),
        "should strip ok summary"
    );
    assert!(result.contains("--- FAIL"), "should keep FAIL");
    assert!(
        result.contains("expected 3, got 2"),
        "should keep error details"
    );
    assert!(
        result.contains("FAIL    example"),
        "should keep FAIL summary"
    );
}

// ── Filesystem tool tests ──────────────────────────────────────────────────

#[test]
fn ls_strips_os_artifacts_and_dev_dirs() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "ls/ls").unwrap();

    let input = "\
total 48
.
..
.DS_Store
Thumbs.db
desktop.ini
.git
node_modules
target
__pycache__
.next
src
Cargo.toml
README.md";

    let result = apply_filters(input, &config.optimize);
    assert!(!result.contains("total 48"), "should strip total header");
    assert!(!result.contains(".DS_Store"), "should strip .DS_Store");
    assert!(!result.contains("Thumbs.db"), "should strip Thumbs.db");
    assert!(
        !result.contains("node_modules"),
        "should strip node_modules"
    );
    assert!(!result.contains("target"), "should strip target");
    assert!(!result.contains("__pycache__"), "should strip __pycache__");
    assert!(result.contains("src"), "should keep src");
    assert!(result.contains("Cargo.toml"), "should keep Cargo.toml");
}

#[test]
fn ls_transform_adds_one_per_line() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "ls/ls").unwrap();
    assert!(
        config.transform.add.iter().any(|f| f.contains("-1")),
        "ls should add -1"
    );
}

#[test]
fn ls_replace_strips_long_format_metadata() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "ls/ls").unwrap();
    assert!(
        !config.optimize.replace.is_empty(),
        "ls should have replace rules"
    );

    let re = Regex::new(&config.optimize.replace[0].pattern).unwrap();
    let line = "drwxr-xr-x  5 luca  staff  160B Feb 16 14:30 src";
    assert!(re.is_match(line), "should match long format line");
}

#[test]
fn tree_transform_adds_gitignore_and_noreport() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "tree/tree").unwrap();
    assert!(
        config
            .transform
            .add
            .iter()
            .any(|f| f.contains("--gitignore")),
        "tree should add --gitignore"
    );
    assert!(
        config
            .transform
            .add
            .iter()
            .any(|f| f.contains("--noreport")),
        "tree should add --noreport"
    );
}

#[test]
fn find_strips_noise_dirs_and_permission_errors() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "find/find").unwrap();

    let input = "\
./src/main.rs
./node_modules/express/index.js
./.git/HEAD
./.git
./__pycache__/foo.pyc
./target/debug/tkn
./.DS_Store
./Cargo.toml
find: ./private: Permission denied";

    let result = apply_filters(input, &config.optimize);
    assert!(
        !result.contains("node_modules"),
        "should strip node_modules"
    );
    assert!(!result.contains(".git/HEAD"), "should strip .git/");
    assert!(!result.contains("__pycache__"), "should strip __pycache__");
    assert!(!result.contains("target/debug"), "should strip target/");
    assert!(!result.contains(".DS_Store"), "should strip .DS_Store");
    assert!(
        !result.contains("Permission denied"),
        "should strip permission errors"
    );
    assert!(result.contains("./src/main.rs"), "should keep source files");
    assert!(result.contains("./Cargo.toml"), "should keep project files");
}

// ── Search tools tests ─────────────────────────────────────────────────────

#[test]
fn rg_transform_adds_no_heading_and_no_color() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "rg/rg").unwrap();
    assert!(config
        .transform
        .add
        .iter()
        .any(|f| f.contains("--no-heading")),);
    assert!(config
        .transform
        .add
        .iter()
        .any(|f| f.contains("--color=never")),);
}

#[test]
fn rg_strips_binary_matches() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "rg/rg").unwrap();

    let input = "\
src/main.rs:5:fn main() {
Binary file target/debug/tkn matches
--
src/lib.rs:10:pub fn run()";

    let result = apply_filters(input, &config.optimize);
    assert!(
        !result.contains("Binary file"),
        "should strip binary matches"
    );
    assert!(!result.contains("--"), "should strip separators");
    assert!(result.contains("fn main()"), "should keep matches");
}

#[test]
fn grep_has_config() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "grep/grep").unwrap();
    assert_eq!(config.match_pattern, "grep");
}

// ── Network tool tests ─────────────────────────────────────────────────────

#[test]
fn curl_transform_adds_silent() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "curl/curl").unwrap();
    assert!(
        config.transform.add.iter().any(|f| f.contains("-s")),
        "curl should add -s"
    );
}

#[test]
fn curl_strips_progress_bars() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "curl/curl").unwrap();

    let input = "\
  % Total    % Received % Xferd  Average Speed
  0 12345    0     0    0     0      0 --:--:-- --:--:-- --:--:--     0
{\"status\": \"ok\"}";

    let result = apply_filters(input, &config.optimize);
    assert!(result.contains("\"status\""), "should keep response body");
    assert!(!result.contains("% Total"), "should strip progress header");
}

// ── GitHub CLI tests ───────────────────────────────────────────────────────

#[test]
fn gh_pr_strips_noise() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "gh/pr").unwrap();

    let input = "\
Title: Fix login bug
—————————————————
Status: Open
View this pull request on GitHub:
https://github.com/org/repo/pull/42
No description provided.
  Co-Authored-By: Claude <noreply@anthropic.com>
  Generated with Claude Code
Checks passing";

    let result = apply_filters(input, &config.optimize);
    assert!(result.contains("Title: Fix login bug"), "should keep title");
    assert!(result.contains("Status: Open"), "should keep status");
    assert!(result.contains("Checks passing"), "should keep checks");
    assert!(
        !result.contains("View this pull request"),
        "should strip view link"
    );
    assert!(!result.contains("https://github.com"), "should strip URL");
    assert!(
        !result.contains("No description provided"),
        "should strip empty desc"
    );
    assert!(!result.contains("Co-Authored-By"), "should strip co-author");
}

// ── Kubectl tests ──────────────────────────────────────────────────────────

#[test]
fn kubectl_get_strips_empty_and_no_resources() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "kubectl/get").unwrap();

    let input = "\
NAME          READY   STATUS    RESTARTS   AGE
nginx-pod     1/1     Running   0          5d
No resources found in default namespace.

redis-pod     1/1     Running   0          3d";

    let result = apply_filters(input, &config.optimize);
    assert!(result.contains("nginx-pod"), "should keep pods");
    assert!(
        !result.contains("No resources found"),
        "should strip no-resources"
    );
}

#[test]
fn kubectl_get_replace_compacts_columns() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "kubectl/get").unwrap();
    let replace = &config.optimize.replace;
    assert!(!replace.is_empty());

    let re = Regex::new(&replace[0].pattern).unwrap();
    let line = "nginx-pod     1/1     Running   0          5d";
    let result = re.replace_all(line, &replace[0].replacement);
    // Multi-space runs should be compacted to tabs
    assert!(result.contains('\t'), "should compact spaces to tabs");
    assert!(
        !result.contains("     "),
        "should not have multi-space runs"
    );
}

// ── Swift/Xcode plugin tests ──────────────────────────────────────────────

#[test]
fn swift_build_has_config() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "swift/build").unwrap();
    assert_eq!(config.match_pattern, "swift build");
}

#[test]
fn xcodebuild_test_has_config() {
    let plugins = load_all_plugins();
    let (_, config) = plugins
        .iter()
        .find(|(n, _)| n == "xcodebuild/test")
        .unwrap();
    assert_eq!(config.match_pattern, "xcodebuild test");
}

// ── Biome plugin tests ─────────────────────────────────────────────────────

#[test]
fn biome_check_has_config() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "biome/check").unwrap();
    assert_eq!(config.match_pattern, "biome check");
}

#[test]
fn biome_lint_has_config() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "biome/lint").unwrap();
    assert_eq!(config.match_pattern, "biome lint");
}

// ── Wrangler plugin tests ──────────────────────────────────────────────────

#[test]
fn wrangler_deploy_has_config() {
    let plugins = load_all_plugins();
    let (_, config) = plugins
        .iter()
        .find(|(n, _)| n == "wrangler/deploy")
        .unwrap();
    assert_eq!(config.match_pattern, "wrangler deploy");
}

// ── Deno plugin tests ──────────────────────────────────────────────────────

#[test]
fn deno_test_has_config() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "deno/test").unwrap();
    assert_eq!(config.match_pattern, "deno test");
}

// ── pnpm plugin tests ──────────────────────────────────────────────────────

#[test]
fn pnpm_install_has_config() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "pnpm/install").unwrap();
    assert_eq!(config.match_pattern, "pnpm install");
}

// ── Miscellaneous tool tests ───────────────────────────────────────────────

#[test]
fn make_has_config() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "make/make").unwrap();
    assert_eq!(config.match_pattern, "make");
}

#[test]
fn cat_has_config() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "cat/cat").unwrap();
    assert_eq!(config.match_pattern, "cat");
}

#[test]
fn sed_has_config() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "sed/sed").unwrap();
    assert_eq!(config.match_pattern, "sed");
}

#[test]
fn wc_has_config() {
    let plugins = load_all_plugins();
    let (_, config) = plugins.iter().find(|(n, _)| n == "wc/wc").unwrap();
    assert_eq!(config.match_pattern, "wc");
}

// ── Exhaustive coverage: every plugin file loads ──────────────────────────

#[test]
fn all_expected_plugins_exist() {
    let plugins = load_all_plugins();
    let names: Vec<&str> = plugins.iter().map(|(n, _)| n.as_str()).collect();

    let expected = [
        // git
        "git/diff",
        "git/status",
        "git/show",
        "git/log",
        "git/commit",
        "git/branch",
        "git/push",
        "git/pull",
        "git/fetch",
        "git/add",
        "git/blame",
        "git/remote",
        "git/checkout",
        "git/rm",
        "git/worktree",
        "git/stash",
        "git/ls-files",
        // cargo
        "cargo/build",
        "cargo/test",
        "cargo/clippy",
        "cargo/fmt",
        "cargo/help",
        // gh
        "gh/pr",
        "gh/issue",
        "gh/repo",
        "gh/run",
        "gh/api",
        // bun
        "bun/run",
        "bun/test",
        "bun/add",
        "bun/build",
        "bun/install",
        "bun/remove",
        // npm
        "npm/run",
        "npm/test",
        "npm/install",
        "npm/view",
        // pnpm
        "pnpm/install",
        "pnpm/list",
        "pnpm/outdated",
        "pnpm/run",
        // python
        "pytest/pytest",
        "ruff/check",
        "ruff/format",
        "pip/install",
        "pip/list",
        // go
        "go/test",
        "go/build",
        "go/vet",
        // docker
        "docker/ps",
        "docker/images",
        "docker/logs",
        "docker/build",
        "docker/compose",
        "docker/exec",
        // kubectl
        "kubectl/get",
        "kubectl/logs",
        // js/ts linters
        "tsc/tsc",
        "eslint/eslint",
        "prettier/prettier",
        // biome
        "biome/check",
        "biome/format",
        "biome/lint",
        // swift
        "swift/build",
        "swift/test",
        // xcodebuild
        "xcodebuild/build",
        "xcodebuild/test",
        // wrangler
        "wrangler/deploy",
        "wrangler/publish",
        // deno
        "deno/test",
        "deno/bench",
        // filesystem/search
        "find/find",
        "ls/ls",
        "tree/tree",
        "grep/grep",
        "rg/rg",
        "cat/cat",
        "head/head",
        "tail/tail",
        "sed/sed",
        // network
        "curl/curl",
        "wget/wget",
        // misc
        "make/make",
        "du/du",
        "ps/ps",
        "wc/wc",
        "nl/nl",
    ];

    for name in expected {
        assert!(
            names.contains(&name),
            "expected plugin {name} not found in plugins/"
        );
    }
}
