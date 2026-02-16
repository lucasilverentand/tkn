use chrono::Utc;
use uuid::Uuid;

use crate::optimizer;
use crate::runner;
use crate::storage::StorageManager;
use crate::tool_config;
use crate::transformer;
use crate::types::{LogEntry, SessionEntry};

/// Environment variable that carries the original command string verbatim
/// from the hook, bypassing shell arg splitting.
const ENV_ORIGINAL_CMD: &str = "TKN_ORIGINAL_CMD";

pub fn run(args: &[String]) -> i32 {
    // Prefer the env var (set by the hook) so we get the exact command string
    // without shell arg-splitting losing quoting. Fall back to args for direct
    // `tkn exec -- <command>` invocations.
    let command = std::env::var(ENV_ORIGINAL_CMD)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| args.join(" "));

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

    let transformed_command = if actual_command != command {
        Some(actual_command.clone())
    } else {
        None
    };

    let log_entry = LogEntry {
        ref_id: ref_id.clone(),
        command: command.clone(),
        transformed_command: transformed_command.clone(),
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
        transformed_command,
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
