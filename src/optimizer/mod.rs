mod basic;

use regex::Regex;

use crate::tool_config::ToolConfig;
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

        // Apply plugin's max_bytes truncation if set
        if let Some(max_bytes) = config.optimize.max_bytes {
            if optimized.len() > max_bytes {
                optimized = truncate_middle(&optimized, max_bytes);
                was_truncated = true;
            }
        }
    }

    // Global line cap: truncate to MAX_OUTPUT_LINES (keeping head + tail)
    let (truncated_content, line_truncated) = truncate_lines(&optimized, MAX_OUTPUT_LINES);
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

/// Truncate by removing the middle of the output, keeping the beginning and end
/// which typically carry the most useful signal (headers, context, results, errors).
fn truncate_middle(input: &str, max_bytes: usize) -> String {
    let lines: Vec<&str> = input.lines().collect();

    if lines.len() <= 2 {
        let mut out = input[..max_bytes].to_string();
        if let Some(pos) = out.rfind('\n') {
            out.truncate(pos + 1);
        }
        out.push_str("\n[... truncated ...]");
        return out;
    }

    // Budget: ~40% bytes each for head and tail, capped at 100 lines each
    let half_budget = max_bytes * 2 / 5;
    let max_section_lines = 100;

    // Collect head lines up to budget
    let mut head = String::new();
    let mut head_count = 0;
    for line in &lines {
        if head_count >= max_section_lines {
            break;
        }
        let next = if head.is_empty() {
            line.len()
        } else {
            head.len() + 1 + line.len()
        };
        if next > half_budget && head_count > 0 {
            break;
        }
        if !head.is_empty() {
            head.push('\n');
        }
        head.push_str(line);
        head_count += 1;
    }

    // Collect tail lines up to budget (from the end, skip what head already took)
    let mut tail_lines: Vec<&str> = Vec::new();
    let mut tail_bytes = 0;
    for line in lines[head_count..].iter().rev() {
        if tail_lines.len() >= max_section_lines {
            break;
        }
        let next = if tail_bytes == 0 {
            line.len()
        } else {
            tail_bytes + 1 + line.len()
        };
        if next > half_budget && !tail_lines.is_empty() {
            break;
        }
        tail_lines.push(line);
        tail_bytes = next;
    }
    tail_lines.reverse();

    let omitted = lines.len() - head_count - tail_lines.len();
    let separator = format!("\n[... {} lines omitted ...]\n", omitted);

    let mut result = head;
    result.push_str(&separator);
    result.push_str(&tail_lines.join("\n"));
    result
}

/// Truncate by line count, keeping head and tail lines with a separator in the middle.
/// Returns (output, was_truncated).
fn truncate_lines(input: &str, max_lines: usize) -> (String, bool) {
    let lines: Vec<&str> = input.lines().collect();
    if lines.len() <= max_lines {
        return (input.to_string(), false);
    }

    let head_count = max_lines * 2 / 5;
    let tail_count = max_lines * 2 / 5;

    let head = &lines[..head_count];
    let tail = &lines[lines.len() - tail_count..];
    let omitted = lines.len() - head_count - tail_count;

    let mut result = head.join("\n");
    result.push_str(&format!("\n[... {} lines omitted ...]\n", omitted));
    result.push_str(&tail.join("\n"));

    (result, true)
}

fn apply_tool_filters(input: &str, config: &ToolConfig) -> String {
    let opt = &config.optimize;

    if opt.keep.is_empty() && opt.strip.is_empty() {
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

    let filtered: Vec<&str> = input
        .lines()
        .filter(|line| {
            // keep wins: if keep patterns exist, only keep matching lines
            if !keep_regexes.is_empty() {
                return keep_regexes.iter().any(|re| re.is_match(line));
            }
            // Otherwise, strip matching lines
            !strip_regexes.iter().any(|re| re.is_match(line))
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
            keep: vec![],
            max_bytes: None,
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
            strip: vec![],
            keep: vec![r"^\+".into(), r"^@@".into()],
            max_bytes: None,
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
            max_bytes: None,
            ..Default::default()
        });

        let input = "foo line\nbar line\nbaz line";
        let result = apply_tool_filters(input, &config);
        assert_eq!(result, "bar line");
    }

    #[test]
    fn test_custom_max_bytes() {
        let config = config_with_optimize(OptimizeConfig {
            strip: vec![],
            keep: vec![],
            max_bytes: Some(50),
            ..Default::default()
        });

        let input = "a\n".repeat(100);
        let raw = input.as_bytes();
        let result = run_pipeline(raw, Some(&config));
        assert!(result.was_truncated);
        assert!(result.content.contains("omitted"));
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
    fn test_truncate_middle_keeps_head_and_tail() {
        let lines: Vec<String> = (1..=50).map(|i| format!("line {i}")).collect();
        let input = lines.join("\n");
        let result = truncate_middle(&input, 200);
        assert!(result.starts_with("line 1\n"));
        assert!(result.contains("omitted"));
        assert!(result.ends_with("line 50"));
        assert!(!result.contains("line 25"));
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
        let (result, truncated) = truncate_lines(input, 500);
        assert_eq!(result, input);
        assert!(!truncated);
    }

    #[test]
    fn test_truncate_lines_over_limit() {
        let lines: Vec<String> = (1..=600).map(|i| format!("line {i}")).collect();
        let input = lines.join("\n");
        let (result, truncated) = truncate_lines(&input, 500);
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
        let (result, truncated) = truncate_lines(&input, 500);
        assert!(truncated);
        // 40% of 500 = 200 head lines, 200 tail lines
        assert!(result.contains("line 200"));
        assert!(!result.contains("line 201\n"));
        assert!(result.contains("line 801"));
        assert!(result.contains("[... 600 lines omitted ...]"));
    }
}
