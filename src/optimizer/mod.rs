mod basic;

use regex::Regex;

use crate::tool_config::{ToolConfig, TruncateMode};
use crate::types::OptimizedOutput;

pub use basic::strip_ansi;

const MAX_OUTPUT_LINES: usize = 500;

pub fn run_pipeline(raw: &[u8], tool_config: Option<&ToolConfig>) -> OptimizedOutput {
    let raw_str = String::from_utf8_lossy(raw);
    let original_bytes = raw.len();

    // Always: strip ANSI + resolve \r. Unless plugin opts into raw, also trim whitespace + collapse blanks.
    let raw_mode = tool_config.map_or(false, |c| c.optimize.raw);
    let mut optimized = strip_ansi(&raw_str, raw_mode);
    let mut was_truncated = false;

    if let Some(config) = tool_config {
        // Plugin exists → apply its strip/keep filters
        optimized = apply_tool_filters(&optimized, config);
    }

    // Line cap: use plugin's max_lines if set, otherwise the global default
    let line_limit = tool_config
        .and_then(|c| c.optimize.max_lines)
        .unwrap_or(MAX_OUTPUT_LINES);
    let truncate_mode = tool_config
        .map(|c| c.optimize.truncate)
        .unwrap_or_default();
    let (truncated_content, line_truncated) = truncate_lines(&optimized, line_limit, truncate_mode);
    optimized = truncated_content;
    was_truncated = was_truncated || line_truncated;

    let optimized_bytes = optimized.len();

    OptimizedOutput {
        content: optimized,
        original_bytes,
        optimized_bytes,
        was_truncated,
    }
}

/// Truncate by line count using the specified strategy.
/// Returns (output, was_truncated).
fn truncate_lines(input: &str, max_lines: usize, mode: TruncateMode) -> (String, bool) {
    let lines: Vec<&str> = input.lines().collect();
    if lines.len() <= max_lines {
        return (input.to_string(), false);
    }

    let omitted = lines.len() - max_lines;

    match mode {
        TruncateMode::Top => {
            let mut result = lines[..max_lines].join("\n");
            result.push_str(&format!("\n[... {} lines omitted ...]", omitted));
            (result, true)
        }
        TruncateMode::Middle => {
            let head_count = max_lines * 2 / 5;
            let tail_count = max_lines * 2 / 5;
            let omitted = lines.len() - head_count - tail_count;

            let mut result = lines[..head_count].join("\n");
            result.push_str(&format!("\n[... {} lines omitted ...]\n", omitted));
            result.push_str(&lines[lines.len() - tail_count..].join("\n"));
            (result, true)
        }
        TruncateMode::Bottom => {
            let mut result = format!("[... {} lines omitted ...]\n", omitted);
            result.push_str(&lines[lines.len() - max_lines..].join("\n"));
            (result, true)
        }
    }
}

fn apply_tool_filters(input: &str, config: &ToolConfig) -> String {
    let opt = &config.optimize;

    if opt.keep.is_empty() && opt.strip.is_empty() && opt.replace.is_empty() {
        return input.to_string();
    }

    let keep_regexes: Vec<Regex> = opt
        .keep
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect();

    let strip_regexes: Vec<Regex> = opt
        .strip
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect();

    let replace_regexes: Vec<(Regex, &str)> = opt
        .replace
        .iter()
        .filter_map(|r| Regex::new(&r.pattern).ok().map(|re| (re, r.replacement.as_str())))
        .collect();

    let filtered: Vec<String> = input
        .lines()
        .filter(|line| {
            // keep wins: if keep patterns exist, only keep matching lines
            if !keep_regexes.is_empty() {
                return keep_regexes.iter().any(|re| re.is_match(line));
            }
            // Otherwise, strip matching lines
            !strip_regexes.iter().any(|re| re.is_match(line))
        })
        .map(|line| {
            if replace_regexes.is_empty() {
                return line.to_string();
            }
            let mut result = line.to_string();
            for (re, replacement) in &replace_regexes {
                result = re.replace_all(&result, *replacement).into_owned();
            }
            result
        })
        .collect();

    filtered.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_config::{OptimizeConfig, ToolConfig, TransformConfig};

    fn config_with_optimize(opt: OptimizeConfig) -> ToolConfig {
        ToolConfig {
            match_pattern: "test".into(),
            transform: TransformConfig::default(),
            optimize: opt,
        }
    }

    #[test]
    fn test_strip_lines() {
        let config = config_with_optimize(OptimizeConfig {
            strip: vec![r"^index [a-f0-9]+".into(), r"^diff --git".into()],
            ..Default::default()
        });

        let input = "diff --git a/foo b/foo\nindex abc123..def456 100644\n--- a/foo\n+++ b/foo\n+new line";
        let result = apply_tool_filters(input, &config);
        assert!(!result.contains("diff --git"));
        assert!(!result.contains("index abc123"));
        assert!(result.contains("--- a/foo"));
        assert!(result.contains("+new line"));
    }

    #[test]
    fn test_keep_lines() {
        let config = config_with_optimize(OptimizeConfig {
            keep: vec![r"^\+".into(), r"^@@".into()],
            ..Default::default()
        });

        let input = "diff --git a/foo b/foo\n@@ -1,3 +1,4 @@\n context\n+added line\n-removed line";
        let result = apply_tool_filters(input, &config);
        assert!(result.contains("@@ -1,3 +1,4 @@"));
        assert!(result.contains("+added line"));
        assert!(!result.contains("diff --git"));
        assert!(!result.contains("context"));
    }

    #[test]
    fn test_keep_wins_over_strip() {
        let config = config_with_optimize(OptimizeConfig {
            strip: vec![r"foo".into()],
            keep: vec![r"bar".into()],
            ..Default::default()
        });

        let input = "foo line\nbar line\nbaz line";
        let result = apply_tool_filters(input, &config);
        assert_eq!(result, "bar line");
    }

    #[test]
    fn test_custom_max_lines() {
        let config = config_with_optimize(OptimizeConfig {
            max_lines: Some(10),
            ..Default::default()
        });

        let input = "a\n".repeat(100);
        let raw = input.as_bytes();
        let result = run_pipeline(raw, Some(&config));
        assert!(result.was_truncated);
        assert!(result.content.contains("omitted"));
    }

    #[test]
    fn test_max_lines_allows_longer_output() {
        let config = config_with_optimize(OptimizeConfig {
            max_lines: Some(1000),
            ..Default::default()
        });

        let input = "a\n".repeat(600);
        let raw = input.as_bytes();
        let result = run_pipeline(raw, Some(&config));
        // 600 lines is under the plugin's 1000 limit, so no truncation
        assert!(!result.was_truncated);
    }

    #[test]
    fn test_no_tool_config_under_line_cap() {
        // Without a plugin, short output passes through without truncation
        let input = "a\n".repeat(100);
        let raw = input.as_bytes();
        let result = run_pipeline(raw, None);
        assert!(!result.was_truncated);
    }

    #[test]
    fn test_no_tool_config_over_line_cap() {
        // Without a plugin, output over 500 lines still gets line-capped
        let input = "a\n".repeat(1000);
        let raw = input.as_bytes();
        let result = run_pipeline(raw, None);
        assert!(result.was_truncated);
        assert!(result.content.contains("lines omitted"));
    }

    #[test]
    fn test_no_plugin_passthrough() {
        let input = "hello\nworld\n";
        let result = run_pipeline(input.as_bytes(), None);
        assert_eq!(result.content, "hello\nworld");
        assert!(!result.was_truncated);
    }

    #[test]
    fn test_no_plugin_strips_ansi() {
        let input = "\x1b[32mhello\x1b[0m\n";
        let result = run_pipeline(input.as_bytes(), None);
        assert_eq!(result.content, "hello");
    }

    #[test]
    fn test_no_filters_passthrough() {
        let config = config_with_optimize(OptimizeConfig::default());
        let input = "hello\nworld";
        assert_eq!(apply_tool_filters(input, &config), "hello\nworld");
    }

    #[test]
    fn test_truncate_lines_under_limit() {
        let input = "line1\nline2\nline3";
        let (result, truncated) = truncate_lines(input, 500, TruncateMode::Middle);
        assert_eq!(result, input);
        assert!(!truncated);
    }

    #[test]
    fn test_truncate_lines_over_limit() {
        let lines: Vec<String> = (1..=600).map(|i| format!("line {i}")).collect();
        let input = lines.join("\n");
        let (result, truncated) = truncate_lines(&input, 500, TruncateMode::Middle);
        assert!(truncated);
        assert!(result.starts_with("line 1\n"));
        assert!(result.contains("lines omitted"));
        assert!(result.ends_with("line 600"));
        // Should not contain lines from the middle
        assert!(!result.contains("line 300"));
    }

    #[test]
    fn test_truncate_lines_preserves_head_tail() {
        let lines: Vec<String> = (1..=1000).map(|i| format!("line {i}")).collect();
        let input = lines.join("\n");
        let (result, truncated) = truncate_lines(&input, 500, TruncateMode::Middle);
        assert!(truncated);
        // 40% of 500 = 200 head lines, 200 tail lines
        assert!(result.contains("line 200"));
        assert!(!result.contains("line 201\n"));
        assert!(result.contains("line 801"));
        assert!(result.contains("[... 600 lines omitted ...]"));
    }

    #[test]
    fn test_truncate_top_keeps_first_lines() {
        let lines: Vec<String> = (1..=100).map(|i| format!("line {i}")).collect();
        let input = lines.join("\n");
        let (result, truncated) = truncate_lines(&input, 10, TruncateMode::Top);
        assert!(truncated);
        assert!(result.starts_with("line 1\n"));
        assert!(result.contains("line 10\n"));
        assert!(!result.contains("line 11"));
        assert!(result.ends_with("[... 90 lines omitted ...]"));
    }

    #[test]
    fn test_truncate_bottom_keeps_last_lines() {
        let lines: Vec<String> = (1..=100).map(|i| format!("line {i}")).collect();
        let input = lines.join("\n");
        let (result, truncated) = truncate_lines(&input, 10, TruncateMode::Bottom);
        assert!(truncated);
        assert!(result.starts_with("[... 90 lines omitted ...]\n"));
        assert!(result.contains("line 91\n"));
        assert!(result.ends_with("line 100"));
        assert!(!result.contains("line 90\n"));
    }

    #[test]
    fn test_replace_inline() {
        use crate::tool_config::ReplaceRule;
        let config = config_with_optimize(OptimizeConfig {
            replace: vec![ReplaceRule {
                pattern: r"^\d+\s+".to_string(),
                replacement: String::new(),
            }],
            ..Default::default()
        });

        let input = "42 hello\n7 world\nno match";
        let result = apply_tool_filters(input, &config);
        assert_eq!(result, "hello\nworld\nno match");
    }

    #[test]
    fn test_replace_with_strip() {
        use crate::tool_config::ReplaceRule;
        let config = config_with_optimize(OptimizeConfig {
            strip: vec![r"^total \d+".to_string()],
            replace: vec![ReplaceRule {
                pattern: r"^[d-][rwx-]{9}\s+\d+\s+\S+\s+\S+\s+".to_string(),
                replacement: String::new(),
            }],
            ..Default::default()
        });

        let input = "total 48\ndrwxr-xr-x  5 luca  staff  160B Feb 16 14:30 src\n-rw-r--r--  1 luca  staff  2.3K Feb 16 14:30 Cargo.toml";
        let result = apply_tool_filters(input, &config);
        assert_eq!(result, "160B Feb 16 14:30 src\n2.3K Feb 16 14:30 Cargo.toml");
    }

    #[test]
    fn test_replace_ordered() {
        use crate::tool_config::ReplaceRule;
        let config = config_with_optimize(OptimizeConfig {
            replace: vec![
                ReplaceRule {
                    pattern: r"foo".to_string(),
                    replacement: "bar".to_string(),
                },
                ReplaceRule {
                    pattern: r"bar".to_string(),
                    replacement: "baz".to_string(),
                },
            ],
            ..Default::default()
        });

        // "foo" → "bar" → "baz" (replacements are ordered/chained)
        let result = apply_tool_filters("foo", &config);
        assert_eq!(result, "baz");
    }
}
