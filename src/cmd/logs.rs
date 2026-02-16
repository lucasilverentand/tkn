use crate::storage::StorageManager;
use crate::tool_config;

pub fn run(id: Option<&str>, lines: Option<&str>) {
    let storage = StorageManager::new();

    match id {
        Some(ref_id) => show_log(&storage, ref_id, lines),
        None => list_logs(&storage),
    }
}

/// Parse a line spec: "42" for a single line, "10:20" for a range.
/// Returns (start, end) as 0-based indices suitable for slicing.
fn parse_line_spec(spec: &str) -> Result<(usize, Option<usize>), String> {
    if let Some((start, end)) = spec.split_once(':') {
        let s: usize = start.parse().map_err(|_| format!("invalid start line: {start}"))?;
        let e: usize = end.parse().map_err(|_| format!("invalid end line: {end}"))?;
        if s == 0 || e == 0 {
            return Err("line numbers are 1-based".to_string());
        }
        if s > e {
            return Err(format!("start ({s}) must be <= end ({e})"));
        }
        Ok((s - 1, Some(e)))
    } else {
        let n: usize = spec.parse().map_err(|_| format!("invalid line number: {spec}"))?;
        if n == 0 {
            return Err("line numbers are 1-based".to_string());
        }
        Ok((n - 1, Some(n)))
    }
}

fn show_log(storage: &StorageManager, ref_id: &str, lines: Option<&str>) {
    match storage.read_log_entry(ref_id) {
        Ok(entry) => {
            // Track that the full log was read (optimizer quality signal)
            let patterns = tool_config::collect_patterns();
            let _ = storage.record_full_log_read(&entry.command, &patterns);

            // Show transformation header when command was transformed
            if let Some(ref transformed) = entry.transformed_command {
                eprintln!("ORG: {}", entry.command);
                eprintln!("NEW: {}", transformed);
                eprintln!();
            }
        }
        Err(e) => {
            eprintln!("tkn: failed to read metadata for {ref_id}: {e}");
        }
    }

    match storage.read_log(ref_id) {
        Ok(content) => {
            if let Some(spec) = lines {
                match parse_line_spec(spec) {
                    Ok((start, end)) => {
                        let all_lines: Vec<&str> = content.lines().collect();
                        let end = end.unwrap_or(all_lines.len()).min(all_lines.len());
                        let start = start.min(all_lines.len());
                        for line in &all_lines[start..end] {
                            println!("{line}");
                        }
                    }
                    Err(e) => eprintln!("tkn: {e}"),
                }
            } else {
                print!("{content}");
            }
        }
        Err(e) => eprintln!("tkn: failed to read log for {ref_id}: {e}"),
    }
}

fn list_logs(storage: &StorageManager) {
    match storage.list_log_entries() {
        Ok(entries) if entries.is_empty() => {
            println!("No logs found.");
        }
        Ok(entries) => {
            println!(
                "{:<16} {:<8} {:<12} {:<10} {}",
                "REF", "EXIT", "SIZE", "SAVED", "COMMAND"
            );
            println!("{}", "-".repeat(72));
            for entry in entries.iter().take(20) {
                let saved = if entry.raw_bytes > 0 {
                    let s = entry
                        .raw_bytes
                        .saturating_sub(entry.optimized_bytes);
                    format!("{:.0}%", (s as f64 / entry.raw_bytes as f64) * 100.0)
                } else {
                    "0%".to_string()
                };
                let cmd = if entry.command.len() > 30 {
                    format!("{}...", &entry.command[..27])
                } else {
                    entry.command.clone()
                };
                println!(
                    "{:<16} {:<8} {:<12} {:<10} {}",
                    entry.ref_id,
                    entry.exit_code,
                    format!("{} B", entry.raw_bytes),
                    saved,
                    cmd
                );
            }
        }
        Err(e) => eprintln!("tkn: failed to list logs: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_line() {
        assert_eq!(parse_line_spec("42"), Ok((41, Some(42))));
    }

    #[test]
    fn test_parse_range() {
        assert_eq!(parse_line_spec("10:20"), Ok((9, Some(20))));
    }

    #[test]
    fn test_parse_single_line_1() {
        assert_eq!(parse_line_spec("1"), Ok((0, Some(1))));
    }

    #[test]
    fn test_parse_zero_rejected() {
        assert!(parse_line_spec("0").is_err());
        assert!(parse_line_spec("0:5").is_err());
    }

    #[test]
    fn test_parse_inverted_range() {
        assert!(parse_line_spec("20:10").is_err());
    }

    #[test]
    fn test_parse_invalid() {
        assert!(parse_line_spec("abc").is_err());
        assert!(parse_line_spec("1:abc").is_err());
    }
}
