use std::sync::LazyLock;

use regex::Regex;

static ANSI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[^\[\]][^\x1b]?").unwrap()
});

/// Clean terminal output for display.
///
/// Always strips ANSI escape sequences and resolves carriage-return overwrites.
/// Unless `raw` is true, also trims trailing whitespace per line and collapses
/// 3+ consecutive blank lines into one.
pub fn strip_ansi(input: &str, raw: bool) -> String {
    let ansi_re = &*ANSI_RE;

    let stripped = ansi_re.replace_all(input, "");

    // Handle \r overwrites (progress bars/spinners): keep only text after last \r per line
    let lines: Vec<&str> = stripped
        .lines()
        .map(|line| {
            let l = if let Some(pos) = line.rfind('\r') {
                &line[pos + 1..]
            } else {
                line
            };
            if raw { l } else { l.trim_end() }
        })
        .collect();

    if raw {
        return lines.join("\n");
    }

    // Collapse 3+ consecutive blank lines into 1 blank line
    let mut result = String::with_capacity(input.len());
    let mut blank_count = 0u32;
    for (i, line) in lines.iter().enumerate() {
        if line.is_empty() {
            blank_count += 1;
            if blank_count <= 1 && i > 0 {
                result.push('\n');
            }
        } else {
            if i > 0 {
                result.push('\n');
            }
            blank_count = 0;
            result.push_str(line);
        }
    }

    result.trim().to_string()
}

/// Collapse consecutive duplicate non-blank lines.
///
/// When the same non-empty line appears 3 or more times in a row, keeps the
/// first occurrence and appends `[... repeated N more times]`. Two identical
/// lines in a row are left alone. Empty/blank lines are never collapsed (blank
/// line collapsing is handled separately by `strip_ansi`).
pub fn collapse_duplicate_lines(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut lines = input.lines().peekable();
    let mut first = true;

    while let Some(line) = lines.next() {
        if !first {
            result.push('\n');
        }
        first = false;

        // Empty/blank lines pass through unchanged
        if line.trim().is_empty() {
            result.push_str(line);
            continue;
        }

        // Count consecutive identical lines (we already consumed the first)
        let mut count: usize = 1;
        while lines.peek() == Some(&line) {
            lines.next();
            count += 1;
        }

        result.push_str(line);
        if count >= 3 {
            result.push('\n');
            result.push_str(&format!("[... repeated {} more times]", count - 1));
        } else if count == 2 {
            // Keep both copies
            result.push('\n');
            result.push_str(line);
        }
    }

    result
}

/// Compact JSON output by removing unnecessary whitespace/indentation.
///
/// If the input looks like JSON (starts with `{` or `[` after trimming) and
/// parses successfully, returns a single-line compact representation.
/// Otherwise returns the input unchanged. This is best-effort — never loses data.
pub fn compact_json(input: &str) -> String {
    let trimmed = input.trim();
    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
        return input.to_string();
    }

    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(value) => serde_json::to_string(&value).unwrap_or_else(|_| input.to_string()),
        Err(_) => input.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi() {
        let input = "\x1b[32mhello\x1b[0m world";
        assert_eq!(strip_ansi(input, false), "hello world");
    }

    #[test]
    fn test_collapse_blank_lines() {
        let input = "a\n\n\n\n\nb";
        assert_eq!(strip_ansi(input, false), "a\n\nb");
    }

    #[test]
    fn test_carriage_return() {
        let input = "downloading... 50%\rdownloading... 100%";
        assert_eq!(strip_ansi(input, false), "downloading... 100%");
    }

    #[test]
    fn test_trim_trailing_whitespace() {
        let input = "hello   \nworld   ";
        assert_eq!(strip_ansi(input, false), "hello\nworld");
    }

    #[test]
    fn test_raw_preserves_whitespace_and_blanks() {
        let input = "hello   \n\n\n\n\nworld   ";
        assert_eq!(strip_ansi(input, true), "hello   \n\n\n\n\nworld   ");
    }

    #[test]
    fn test_raw_still_strips_ansi() {
        let input = "\x1b[32mhello\x1b[0m";
        assert_eq!(strip_ansi(input, true), "hello");
    }

    #[test]
    fn test_compact_json_object() {
        let input = r#"{
  "name": "tkn",
  "version": "0.1.0",
  "description": "Shell proxy"
}"#;
        let result = compact_json(input);
        // serde_json::Value uses BTreeMap so keys are sorted alphabetically
        assert_eq!(
            result,
            r#"{"description":"Shell proxy","name":"tkn","version":"0.1.0"}"#
        );
        // Verify no whitespace/newlines remain
        assert!(!result.contains('\n'));
        assert!(!result.contains("  "));
    }

    #[test]
    fn test_compact_json_array() {
        let input = r#"[
  {
    "id": 1,
    "title": "issue one"
  },
  {
    "id": 2,
    "title": "issue two"
  }
]"#;
        let result = compact_json(input);
        assert_eq!(
            result,
            r#"[{"id":1,"title":"issue one"},{"id":2,"title":"issue two"}]"#
        );
    }

    #[test]
    fn test_compact_json_non_json_unchanged() {
        let input = "hello world\nthis is plain text";
        assert_eq!(compact_json(input), input);
    }

    #[test]
    fn test_compact_json_invalid_json_unchanged() {
        let input = "{ this is not valid json }";
        assert_eq!(compact_json(input), input);
    }

    #[test]
    fn test_compact_json_already_compact() {
        let input = r#"{"name":"tkn","version":"0.1.0"}"#;
        let result = compact_json(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_collapse_duplicate_3_plus() {
        let input = "test foo ... ok\ntest foo ... ok\ntest foo ... ok\ntest foo ... ok";
        let result = collapse_duplicate_lines(input);
        assert_eq!(result, "test foo ... ok\n[... repeated 3 more times]");
    }

    #[test]
    fn test_collapse_duplicate_exactly_3() {
        let input = "same line\nsame line\nsame line";
        let result = collapse_duplicate_lines(input);
        assert_eq!(result, "same line\n[... repeated 2 more times]");
    }

    #[test]
    fn test_collapse_duplicate_2_left_alone() {
        let input = "line a\nline a\nline b";
        let result = collapse_duplicate_lines(input);
        assert_eq!(result, "line a\nline a\nline b");
    }

    #[test]
    fn test_collapse_duplicate_empty_lines_not_collapsed() {
        let input = "a\n\n\n\n\nb";
        let result = collapse_duplicate_lines(input);
        assert_eq!(result, "a\n\n\n\n\nb");
    }

    #[test]
    fn test_collapse_duplicate_mixed_content() {
        let input = "header\nok\nok\nok\nok\nfooter\nunique\nunique";
        let result = collapse_duplicate_lines(input);
        assert_eq!(
            result,
            "header\nok\n[... repeated 3 more times]\nfooter\nunique\nunique"
        );
    }

    #[test]
    fn test_collapse_duplicate_non_consecutive_not_collapsed() {
        let input = "line a\nline b\nline a\nline b\nline a";
        let result = collapse_duplicate_lines(input);
        assert_eq!(result, "line a\nline b\nline a\nline b\nline a");
    }
}
