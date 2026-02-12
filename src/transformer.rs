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

    // 3. Add: append flags that aren't already present
    for flag in &transform.add {
        if !parts.contains(flag) {
            parts.push(flag.clone());
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
        ToolConfig {
            match_pattern: "test".into(),
            transform: TransformConfig {
                add: add.into_iter().map(String::from).collect(),
                remove: remove.into_iter().map(String::from).collect(),
                replace: replace.into_iter().map(|(k, v)| (k.into(), v.into())).collect(),
            },
            optimize: OptimizeConfig::default(),
        }
    }

    #[test]
    fn test_add_flag() {
        let config = make_config(vec!["--no-color"], vec![], vec![]);
        assert_eq!(
            transform_command("git diff src/main.rs", &config),
            "git diff src/main.rs --no-color"
        );
    }

    #[test]
    fn test_add_flag_already_present() {
        let config = make_config(vec!["--no-color"], vec![], vec![]);
        assert_eq!(
            transform_command("git diff --no-color src/main.rs", &config),
            "git diff --no-color src/main.rs"
        );
    }

    #[test]
    fn test_remove_flag() {
        let config = make_config(vec![], vec!["--color"], vec![]);
        assert_eq!(
            transform_command("git diff --color src/main.rs", &config),
            "git diff src/main.rs"
        );
    }

    #[test]
    fn test_replace_flag() {
        let config = make_config(vec![], vec![], vec![("--color=auto", "--color=never")]);
        assert_eq!(
            transform_command("git diff --color=auto src/main.rs", &config),
            "git diff --color=never src/main.rs"
        );
    }

    #[test]
    fn test_combined_remove_replace_add() {
        let config = make_config(
            vec!["--no-color"],
            vec!["--verbose"],
            vec![("--color=auto", "--color=never")],
        );
        let result = transform_command("git diff --verbose --color=auto src/main.rs", &config);
        assert_eq!(result, "git diff --color=never src/main.rs --no-color");
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
    fn test_shell_split_quoted() {
        let parts = shell_split("git commit -m 'hello world'");
        assert_eq!(parts, vec!["git", "commit", "-m", "'hello world'"]);
    }
}
