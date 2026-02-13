use super::Optimizer;

/// Collapses runs of similar consecutive lines into a summary.
///
/// "Similar" means lines that share the same prefix up to the first
/// divergence point (at least `MIN_PREFIX` chars must match). This
/// catches patterns like repeated warnings, log lines, or compiler
/// messages that differ only in a trailing detail.
pub struct DedupOptimizer {
    /// Minimum run length before collapsing kicks in.
    threshold: usize,
}

/// Lines must share at least this many leading characters to be
/// considered "similar".
const MIN_PREFIX: usize = 12;

impl DedupOptimizer {
    pub fn new() -> Self {
        Self { threshold: 3 }
    }
}

/// Return the length of the shared leading byte prefix between two strings.
fn common_prefix_len(a: &str, b: &str) -> usize {
    a.bytes()
        .zip(b.bytes())
        .take_while(|(x, y)| x == y)
        .count()
}

impl Optimizer for DedupOptimizer {
    fn optimize(&self, raw: &str) -> String {
        let lines: Vec<&str> = raw.lines().collect();
        if lines.len() < self.threshold {
            return raw.to_string();
        }

        let mut result = String::with_capacity(raw.len());
        let mut i = 0;

        while i < lines.len() {
            let anchor = lines[i];

            // Don't try to group very short or blank lines — they match
            // too easily and collapsing them removes meaningful structure.
            if anchor.len() < MIN_PREFIX {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(anchor);
                i += 1;
                continue;
            }

            // Count how many subsequent lines are "similar" to the anchor.
            let mut run_end = i + 1;
            while run_end < lines.len()
                && common_prefix_len(anchor, lines[run_end]) >= MIN_PREFIX
            {
                run_end += 1;
            }

            let run_len = run_end - i;

            if run_len >= self.threshold {
                // Keep first line, collapse the rest.
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(anchor);
                let collapsed = run_len - 1;
                result.push_str(&format!("\n[... {collapsed} similar lines ...]"));
            } else {
                // Not enough to collapse — emit all lines verbatim.
                for line in &lines[i..run_end] {
                    if !result.is_empty() {
                        result.push('\n');
                    }
                    result.push_str(line);
                }
            }

            i = run_end;
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collapses_repeated_warnings() {
        let opt = DedupOptimizer::new();
        let input = "\
npm WARN deprecated foo@1.0.0
npm WARN deprecated bar@2.0.0
npm WARN deprecated baz@3.0.0
npm WARN deprecated qux@4.0.0
done";
        let result = opt.optimize(input);
        assert!(result.contains("npm WARN deprecated foo@1.0.0"));
        assert!(result.contains("[... 3 similar lines ...]"));
        assert!(result.contains("done"));
        assert!(!result.contains("bar@2.0.0"));
    }

    #[test]
    fn test_no_collapse_below_threshold() {
        let opt = DedupOptimizer::new();
        let input = "\
npm WARN deprecated foo@1.0.0
npm WARN deprecated bar@2.0.0
done";
        let result = opt.optimize(input);
        assert!(result.contains("foo@1.0.0"));
        assert!(result.contains("bar@2.0.0"));
        assert!(!result.contains("similar lines"));
    }

    #[test]
    fn test_short_lines_not_grouped() {
        let opt = DedupOptimizer::new();
        let input = "a\na\na\na\na\nb";
        let result = opt.optimize(input);
        // Short lines should pass through unchanged
        assert_eq!(result, input);
    }

    #[test]
    fn test_exact_duplicates() {
        let opt = DedupOptimizer::new();
        let input = "\
Compiling mycrate v0.1.0
Compiling mycrate v0.1.0
Compiling mycrate v0.1.0
Compiling mycrate v0.1.0
Finished";
        let result = opt.optimize(input);
        assert!(result.contains("Compiling mycrate v0.1.0"));
        assert!(result.contains("[... 3 similar lines ...]"));
        assert!(result.contains("Finished"));
    }

    #[test]
    fn test_multiple_groups() {
        let opt = DedupOptimizer::new();
        let input = "\
warning: unused import `foo`
warning: unused import `bar`
warning: unused import `baz`
some other line
error: linking failed for `target`
error: linking failed for `target2`
error: linking failed for `target3`
done";
        let result = opt.optimize(input);
        assert_eq!(result.matches("[...").count(), 2);
    }

    #[test]
    fn test_passthrough_no_repetition() {
        let opt = DedupOptimizer::new();
        let input = "line one is unique\nline two is different\nline three stands alone";
        assert_eq!(opt.optimize(input), input);
    }

    #[test]
    fn test_preserves_trailing_newline_style() {
        let opt = DedupOptimizer::new();
        let input = "just one line here";
        assert_eq!(opt.optimize(input), input);
    }
}
