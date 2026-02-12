use chrono::Utc;
use uuid::Uuid;

use crate::optimizer;
use crate::runner;
use crate::storage::StorageManager;
use crate::tool_config;
use crate::transformer;
use crate::types::{LogEntry, SessionEntry};

pub fn run(args: &[String]) -> i32 {
    let command = args.join(" ");
    if command.is_empty() {
        eprintln!("tkn: no command provided");
        return 1;
    }

    // Load tool config and transform command if rules exist
    let config = tool_config::load_tool_config(&command);
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

    let optimized = optimizer::run_pipeline(&raw_output, &ref_id, config.as_ref());

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
    if let Err(e) = storage.update_analytics(&log_entry) {
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

    // Print header + optimized output
    eprintln!("[full output: tkn log {ref_id}]");

    if !optimized.content.is_empty() {
        print!("{}", optimized.content);
        // Ensure trailing newline
        if !optimized.content.ends_with('\n') {
            println!();
        }
    }

    result.exit_code
}
