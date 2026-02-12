use regex::Regex;

use super::Optimizer;

pub struct BasicOptimizer {
    ansi_re: Regex,
}

impl BasicOptimizer {
    pub fn new() -> Self {
        Self {
            // Matches ANSI escape sequences: CSI sequences, OSC sequences, and simple escapes
            ansi_re: Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[^\[\]][^\x1b]?").unwrap(),
        }
    }
}

impl Optimizer for BasicOptimizer {
    fn optimize(&self, raw: &str) -> String {
        let mut s = self.ansi_re.replace_all(raw, "").to_string();

        // Handle \r overwrites (progress bars/spinners): keep only text after last \r per line
        s = s
            .lines()
            .map(|line| {
                if let Some(pos) = line.rfind('\r') {
                    &line[pos + 1..]
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Trim trailing whitespace per line
        s = s.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n");

        // Collapse 3+ consecutive blank lines into 1 blank line
        let lines: Vec<&str> = s.lines().collect();
        let mut result = String::with_capacity(s.len());
        let mut blank_count = 0u32;
        for (i, line) in lines.iter().enumerate() {
            if line.is_empty() {
                blank_count += 1;
                if blank_count <= 1 {
                    if i > 0 {
                        result.push('\n');
                    }
                }
            } else {
                if i > 0 && blank_count == 0 {
                    result.push('\n');
                } else if i > 0 && blank_count > 0 {
                    // blank lines already added a newline, add one more to separate
                    result.push('\n');
                }
                blank_count = 0;
                result.push_str(line);
            }
        }

        // Trim leading/trailing whitespace from entire output
        result.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi() {
        let opt = BasicOptimizer::new();
        let input = "\x1b[32mhello\x1b[0m world";
        assert_eq!(opt.optimize(input), "hello world");
    }

    #[test]
    fn test_collapse_blank_lines() {
        let opt = BasicOptimizer::new();
        let input = "a\n\n\n\n\nb";
        assert_eq!(opt.optimize(input), "a\n\nb");
    }

    #[test]
    fn test_carriage_return() {
        let opt = BasicOptimizer::new();
        let input = "downloading... 50%\rdownloading... 100%";
        assert_eq!(opt.optimize(input), "downloading... 100%");
    }

    #[test]
    fn test_trim_trailing_whitespace() {
        let opt = BasicOptimizer::new();
        let input = "hello   \nworld   ";
        assert_eq!(opt.optimize(input), "hello\nworld");
    }
}
