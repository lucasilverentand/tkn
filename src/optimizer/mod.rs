mod basic;
mod dedup;

use regex::Regex;

use crate::tool_config::ToolConfig;
use crate::types::OptimizedOutput;

pub use basic::BasicOptimizer;
pub use dedup::DedupOptimizer;

pub trait Optimizer {
    fn optimize(&self, raw: &str) -> String;
}

const MAX_OUTPUT_BYTES: usize = 8 * 1024;

const SHORT_LINE_THRESHOLD: usize = 20;

pub fn run_pipeline(raw: &[u8], tool_config: Option<&ToolConfig>) -> OptimizedOutput {
    let raw_str = String::from_utf8_lossy(raw);
    let original_bytes = raw.len();
    let has_plugin = tool_config.is_some();

    // Short output with no specific plugin: pass through as-is
    if !has_plugin && raw_str.lines().count() <= SHORT_LINE_THRESHOLD {
        let content = raw_str.into_owned();
        let optimized_bytes = content.len();
        return OptimizedOutput {
            content,
            original_bytes,
            optimized_bytes,
            was_truncated: false,
        };
    }

    let basic = BasicOptimizer::new();
    let dedup = DedupOptimizer::new();
    let mut optimized = basic.optimize(&raw_str);
    optimized = dedup.optimize(&optimized);

    // Apply tool-specific strip/keep filters
    if let Some(config) = tool_config {
        optimized = apply_tool_filters(&optimized, config);
    }

    let max_bytes = tool_config
        .and_then(|c| c.optimize.max_bytes)
        .unwrap_or(MAX_OUTPUT_BYTES);

    let was_truncated = optimized.len() > max_bytes;
    if was_truncated {
        optimized = truncate_middle(&optimized, max_bytes);
    }

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
        });

        let input = "a\n".repeat(100);
        let raw = input.as_bytes();
        let result = run_pipeline(raw, Some(&config));
        assert!(result.was_truncated);
        assert!(result.content.contains("omitted"));
    }

    #[test]
    fn test_no_tool_config_uses_default_max() {
        let input = "a\n".repeat(10000);
        let raw = input.as_bytes();
        let result = run_pipeline(raw,None);
        // Default is 8KB, input is ~20KB
        assert!(result.was_truncated);
    }

    #[test]
    fn test_truncate_middle_keeps_head_and_tail() {
        let lines: Vec<String> = (1..=50).map(|i| format!("line {i}")).collect();
        let input = lines.join("\n");
        let result = truncate_middle(&input, 200);
        assert!(result.starts_with("line 1\n"));
        assert!(result.contains("omitted"));
        assert!(result.ends_with("line 50"));
        // Should not contain middle lines
        assert!(!result.contains("line 25"));
    }

    #[test]
    fn test_no_filters_passthrough() {
        let config = config_with_optimize(OptimizeConfig::default());
        let input = "hello\nworld";
        assert_eq!(apply_tool_filters(input, &config), "hello\nworld");
    }
}
