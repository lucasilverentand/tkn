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

pub fn run_pipeline(raw: &[u8], ref_id: &str, tool_config: Option<&ToolConfig>) -> OptimizedOutput {
    let raw_str = String::from_utf8_lossy(raw);
    let original_bytes = raw.len();

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
        optimized.truncate(max_bytes);
        // Find last newline to avoid cutting mid-line
        if let Some(pos) = optimized.rfind('\n') {
            optimized.truncate(pos + 1);
        }
        optimized.push_str(&format!(
            "\n[... truncated. Full output: tkn log {ref_id} ...]"
        ));
    }

    let optimized_bytes = optimized.len();

    OptimizedOutput {
        content: optimized,
        original_bytes,
        optimized_bytes,
        was_truncated,
    }
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
        let result = run_pipeline(raw, "test_ref", Some(&config));
        assert!(result.was_truncated);
        assert!(result.content.contains("truncated"));
    }

    #[test]
    fn test_no_tool_config_uses_default_max() {
        let input = "a\n".repeat(10000);
        let raw = input.as_bytes();
        let result = run_pipeline(raw, "test_ref", None);
        // Default is 8KB, input is ~20KB
        assert!(result.was_truncated);
    }

    #[test]
    fn test_no_filters_passthrough() {
        let config = config_with_optimize(OptimizeConfig::default());
        let input = "hello\nworld";
        assert_eq!(apply_tool_filters(input, &config), "hello\nworld");
    }
}
