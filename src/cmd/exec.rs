use chrono::Utc;
use uuid::Uuid;

use crate::optimizer;
use crate::runner;
use crate::storage::StorageManager;
use crate::tool_config;
use crate::transformer;
use crate::types::{LogEntry, SessionEntry};

pub fn run(args: &[String]) -> i32 {
    let command = shell_join(args);
    if command.is_empty() {
        eprintln!("tkn: no command provided");
        return 1;
    }

    // Load tool config and transform command if rules exist
    let patterns = tool_config::collect_patterns();
    let config = tool_config::load_tool_config_with_patterns(&command, &patterns);
    let actual_command = match &config {
        Some(cfg) => transformer::transform_command(&command, cfg),
        None => command.clone(),
    };

    let ref_id = Uuid::new_v4().to_string().replace('-', "")[..8].to_string();
    let (result, duration_ms) = runner::run_command(&actual_command);

    // Combine stdout + stderr for optimization
    let mut raw_output = result.stdout.clone();
    if !result.stderr.is_empty() {
        if !raw_output.is_empty() && !raw_output.ends_with(b"\n") {
            raw_output.push(b'\n');
        }
        raw_output.extend_from_slice(&result.stderr);
    }

    let optimized = optimizer::run_pipeline(&raw_output, config.as_ref());

    let storage = StorageManager::new();
    if let Err(e) = storage.init() {
        eprintln!("tkn: failed to init storage: {e}");
    }

    let log_entry = LogEntry {
        ref_id: ref_id.clone(),
        command: command.clone(),
        exit_code: result.exit_code,
        raw_bytes: optimized.original_bytes,
        optimized_bytes: optimized.optimized_bytes,
        timestamp: Utc::now(),
        duration_ms,
    };

    // Write log (best-effort)
    if let Err(e) = storage.write_log(&ref_id, &raw_output, &log_entry) {
        eprintln!("tkn: failed to write log: {e}");
    }

    // Update analytics (best-effort)
    if let Err(e) = storage.update_analytics_with_patterns(&log_entry, &patterns) {
        eprintln!("tkn: failed to update analytics: {e}");
    }

    // Append session entry (best-effort)
    let session_entry = SessionEntry {
        ref_id: ref_id.clone(),
        command,
        exit_code: result.exit_code,
        raw_bytes: optimized.original_bytes,
        optimized_bytes: optimized.optimized_bytes,
        timestamp: log_entry.timestamp,
        duration_ms,
    };
    if let Err(e) = storage.append_session_entry(&session_entry) {
        eprintln!("tkn: failed to append session: {e}");
    }

    // Opportunistic cleanup
    storage.maybe_auto_cleanup();

    // Print optimized output
    if !optimized.content.is_empty() {
        print!("{}", optimized.content);
        if !optimized.content.ends_with('\n') {
            println!();
        }
    }

    // Show footer only when output was meaningfully reduced
    let saved = optimized.original_bytes.saturating_sub(optimized.optimized_bytes);
    let meaningful = saved > 10 && optimized.original_bytes > 0;
    if meaningful && optimized.was_truncated {
        eprintln!("output truncated and optimized, for full output run: tkn log {ref_id}");
    } else if meaningful {
        eprintln!("output optimized, for full output run: tkn log {ref_id}");
    }

    result.exit_code
}

/// Join args into a shell command string, quoting any arg that contains
/// shell-special characters so the command survives re-interpretation by `$SHELL -c`.
fn shell_join(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if arg.is_empty() {
                "''".to_string()
            } else if needs_quoting(arg) {
                // Use single quotes; escape any embedded single quotes as '\''
                format!("'{}'", arg.replace('\'', "'\\''"))
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn needs_quoting(s: &str) -> bool {
    s.chars().any(|c| matches!(c,
        ' ' | '\t' | '\n' | '"' | '\'' | '\\' | '`' | '$' | '!' |
        '&' | '|' | ';' | '(' | ')' | '<' | '>' | '*' | '?' | '[' |
        ']' | '#' | '~' | '{' | '}'
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_join_simple() {
        let args: Vec<String> = vec!["git", "status"].into_iter().map(String::from).collect();
        assert_eq!(shell_join(&args), "git status");
    }

    #[test]
    fn test_shell_join_quotes_spaces() {
        let args: Vec<String> = vec!["git", "commit", "-m", "hello world"]
            .into_iter().map(String::from).collect();
        assert_eq!(shell_join(&args), "git commit -m 'hello world'");
    }

    #[test]
    fn test_shell_join_multiline_message() {
        let args: Vec<String> = vec!["git", "commit", "-m", "feat: add feature\n\nLong description here.\n\nCo-Authored-By: Test"]
            .into_iter().map(String::from).collect();
        let joined = shell_join(&args);
        assert!(joined.starts_with("git commit -m '"));
        assert!(joined.contains("feat: add feature"));
        assert!(joined.ends_with('\''));
    }

    #[test]
    fn test_shell_join_embedded_single_quotes() {
        let args: Vec<String> = vec!["echo", "it's a test"]
            .into_iter().map(String::from).collect();
        assert_eq!(shell_join(&args), "echo 'it'\\''s a test'");
    }

    #[test]
    fn test_shell_join_empty_arg() {
        let args: Vec<String> = vec!["echo", ""].into_iter().map(String::from).collect();
        assert_eq!(shell_join(&args), "echo ''");
    }
}
