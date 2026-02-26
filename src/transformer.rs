use crate::tool_config::ToolConfig;

/// Apply transform rules to a command string before execution.
/// Order: remove → replace → add
pub fn transform_command(command: &str, config: &ToolConfig) -> String {
    let transform = &config.transform;

    if transform.add.is_empty() && transform.remove.is_empty() && transform.replace.is_empty() {
        return command.to_string();
    }

    let mut parts: Vec<String> = shell_split(command);

    // 1. Remove: drop any args that match remove list
    if !transform.remove.is_empty() {
        parts.retain(|p| !transform.remove.contains(p));
    }

    // 2. Replace: swap matching args
    if !transform.replace.is_empty() {
        for part in parts.iter_mut() {
            if let Some(replacement) = transform.replace.get(part.as_str()) {
                *part = replacement.clone();
            }
        }
    }

    // 3. Add: insert flags that aren't already present.
    //    Supports aliases via pipe separator: "--short|-s" means add "--short"
    //    only if neither "--short" nor "-s" is already present.
    //    Short flags (e.g. "-l") are also detected inside combined groups (e.g. "-la").
    //    New flags are inserted before the first positional argument.
    let mut to_add = Vec::new();
    for flag in &transform.add {
        let variants: Vec<&str> = flag.split('|').collect();
        let canonical = variants[0];
        let already_present = variants.iter().any(|v| {
            parts.iter().any(|p| {
                if p == v {
                    return true;
                }
                // Check combined short flags: "-l" matches inside "-la"
                if v.starts_with('-') && !v.starts_with("--") && v.len() == 2 {
                    let ch = v.chars().nth(1).unwrap();
                    if p.starts_with('-') && !p.starts_with("--") && p.len() > 2 {
                        return p[1..].contains(ch);
                    }
                }
                false
            })
        });
        if !already_present {
            to_add.push(canonical.to_string());
        }
    }

    if !to_add.is_empty() {
        // Insert after the last existing flag to keep flags before positional args.
        // When no flags exist, insert right after the command prefix (derived from
        // the plugin match pattern) so that tools like macOS `ls` — which stop
        // parsing options after the first non-flag argument — still see the flags.
        let insert_pos = parts
            .iter()
            .rposition(|p| p.starts_with('-'))
            .map(|i| {
                // If the next token after the flag is a numeric value, it's likely a
                // flag argument (e.g. "-L 3", "--depth 2", "-n 20"). Skip past it
                // so we don't insert new flags between a flag and its value.
                let next = i + 1;
                if next < parts.len()
                    && parts[next].chars().all(|c| c.is_ascii_digit())
                {
                    next + 1
                } else {
                    next
                }
            })
            .unwrap_or_else(|| {
                // No flags present — insert after the command prefix.
                // The match pattern tells us how many leading tokens are
                // "the command" (e.g. "ls" = 1, "git status" = 2).
                let prefix_len = shell_split(&config.match_pattern).len().max(1);
                prefix_len.min(parts.len())
            });
        for (i, flag) in to_add.into_iter().enumerate() {
            parts.insert(insert_pos + i, flag);
        }
    }

    parts.join(" ")
}

/// Simple shell-like splitting that preserves quoted strings.
fn shell_split(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;

    for ch in input.chars() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(ch);
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(ch);
            }
            ' ' | '\t' if !in_single && !in_double => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_config::{TransformConfig, OptimizeConfig};

    fn make_config(
        add: Vec<&str>,
        remove: Vec<&str>,
        replace: Vec<(&str, &str)>,
    ) -> ToolConfig {
        make_config_with_match("test", add, remove, replace)
    }

    fn make_config_with_match(
        match_pattern: &str,
        add: Vec<&str>,
        remove: Vec<&str>,
        replace: Vec<(&str, &str)>,
    ) -> ToolConfig {
        ToolConfig {
            match_pattern: match_pattern.into(),
            transform: TransformConfig {
                add: add.into_iter().map(String::from).collect(),
                remove: remove.into_iter().map(String::from).collect(),
                replace: replace.into_iter().map(|(k, v)| (k.into(), v.into())).collect(),
                savings_factor: None,
            },
            optimize: OptimizeConfig::default(),
        }
    }

    #[test]
    fn test_add_flag() {
        let config = make_config_with_match("git diff", vec!["--no-color"], vec![], vec![]);
        // No existing flags — new flag inserted after command prefix, before positional arg
        assert_eq!(
            transform_command("git diff src/main.rs", &config),
            "git diff --no-color src/main.rs"
        );
    }

    #[test]
    fn test_add_flag_already_present() {
        let config = make_config_with_match("git diff", vec!["--no-color"], vec![], vec![]);
        assert_eq!(
            transform_command("git diff --no-color src/main.rs", &config),
            "git diff --no-color src/main.rs"
        );
    }

    #[test]
    fn test_remove_flag() {
        let config = make_config_with_match("git diff", vec![], vec!["--color"], vec![]);
        assert_eq!(
            transform_command("git diff --color src/main.rs", &config),
            "git diff src/main.rs"
        );
    }

    #[test]
    fn test_replace_flag() {
        let config = make_config_with_match("git diff", vec![], vec![], vec![("--color=auto", "--color=never")]);
        assert_eq!(
            transform_command("git diff --color=auto src/main.rs", &config),
            "git diff --color=never src/main.rs"
        );
    }

    #[test]
    fn test_combined_remove_replace_add() {
        let config = make_config_with_match(
            "git diff",
            vec!["--no-color"],
            vec!["--verbose"],
            vec![("--color=auto", "--color=never")],
        );
        let result = transform_command("git diff --verbose --color=auto src/main.rs", &config);
        // After remove --verbose and replace --color=auto: "git diff --color=never src/main.rs"
        // Then add --no-color after last flag (--color=never)
        assert_eq!(result, "git diff --color=never --no-color src/main.rs");
    }

    #[test]
    fn test_no_transform_rules() {
        let config = make_config(vec![], vec![], vec![]);
        assert_eq!(
            transform_command("echo hello", &config),
            "echo hello"
        );
    }

    #[test]
    fn test_add_flag_with_alias_already_present() {
        let config = make_config_with_match("git status", vec!["--short|-s"], vec![], vec![]);
        // -s is an alias for --short, so --short should not be added
        assert_eq!(
            transform_command("git status -s", &config),
            "git status -s"
        );
    }

    #[test]
    fn test_add_flag_with_alias_canonical_present() {
        let config = make_config_with_match("git status", vec!["--short|-s"], vec![], vec![]);
        assert_eq!(
            transform_command("git status --short", &config),
            "git status --short"
        );
    }

    #[test]
    fn test_add_flag_with_alias_none_present() {
        let config = make_config_with_match("git status", vec!["--short|-s"], vec![], vec![]);
        assert_eq!(
            transform_command("git status", &config),
            "git status --short"
        );
    }

    #[test]
    fn test_shell_split_quoted() {
        let parts = shell_split("git commit -m 'hello world'");
        assert_eq!(parts, vec!["git", "commit", "-m", "'hello world'"]);
    }

    #[test]
    fn test_add_detects_combined_short_flags() {
        // "-1|-l" should not add -1 when -l is inside combined "-la"
        let config = make_config_with_match("ls", vec!["-1|-l"], vec![], vec![]);
        assert_eq!(
            transform_command("ls -la /tmp", &config),
            "ls -la /tmp"
        );
    }

    #[test]
    fn test_add_combined_short_flag_not_present() {
        // "-1|-l" should add -1 when neither -1 nor -l is present
        // No existing flags — inserted after command prefix, before path arg
        let config = make_config_with_match("ls", vec!["-1|-l"], vec![], vec![]);
        assert_eq!(
            transform_command("ls /tmp", &config),
            "ls -1 /tmp"
        );
    }

    #[test]
    fn test_add_inserts_before_positional_args() {
        let config = make_config_with_match("ls", vec!["-h"], vec![], vec![]);
        assert_eq!(
            transform_command("ls -la /tmp", &config),
            "ls -la -h /tmp"
        );
    }

    #[test]
    fn test_add_multiple_flags_after_last_flag() {
        // With existing flags, new flags inserted after last flag
        let config = make_config_with_match("ls", vec!["-1|-l", "-h"], vec![], vec![]);
        assert_eq!(
            transform_command("ls -a /tmp", &config),
            "ls -a -1 -h /tmp"
        );
    }

    #[test]
    fn test_add_flag_no_positional_args() {
        let config = make_config_with_match("ls", vec!["-h"], vec![], vec![]);
        assert_eq!(
            transform_command("ls -la", &config),
            "ls -la -h"
        );
    }

    #[test]
    fn test_add_multiple_flags_no_existing_flags() {
        // Reproduces the `ls plugins/` bug: flags must appear before path args
        // even when the command has no existing flags.
        let config = make_config_with_match("ls", vec!["-1|-l", "-h"], vec![], vec![]);
        assert_eq!(
            transform_command("ls plugins/", &config),
            "ls -1 -h plugins/"
        );
    }

    #[test]
    fn test_add_flag_skips_flag_value() {
        // Flags like -L that take a separate value argument should not be split.
        // "tree -L 1" + adding "--noreport" should NOT produce "tree -L --noreport 1"
        let config = make_config_with_match("tree", vec!["--noreport", "-n|--no-color"], vec![], vec![]);
        assert_eq!(
            transform_command("tree -L 1", &config),
            "tree -L 1 --noreport -n"
        );
    }

    #[test]
    fn test_add_flag_skips_long_flag_value() {
        // Long flags like --depth that take a separate value should not be split.
        let config = make_config_with_match("tree", vec!["--noreport"], vec![], vec![]);
        assert_eq!(
            transform_command("tree --depth 3 src/", &config),
            "tree --depth 3 --noreport src/"
        );
    }

    #[test]
    fn test_add_flag_command_only() {
        // No positional args, no existing flags — flags appended at end
        let config = make_config_with_match("ls", vec!["-1|-l", "-h"], vec![], vec![]);
        assert_eq!(
            transform_command("ls", &config),
            "ls -1 -h"
        );
    }
}
