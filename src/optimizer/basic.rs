use regex::Regex;

/// Clean terminal output for display.
///
/// Always strips ANSI escape sequences and resolves carriage-return overwrites.
/// Unless `raw` is true, also trims trailing whitespace per line and collapses
/// 3+ consecutive blank lines into one.
pub fn strip_ansi(input: &str, raw: bool) -> String {
    let ansi_re =
        Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[^\[\]][^\x1b]?").unwrap();

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
            if i > 0 && blank_count == 0 {
                result.push('\n');
            } else if i > 0 && blank_count > 0 {
                result.push('\n');
            }
            blank_count = 0;
            result.push_str(line);
        }
    }

    result.trim().to_string()
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
}
