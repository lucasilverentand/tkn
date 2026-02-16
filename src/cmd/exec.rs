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

    // Determine whether stderr should go through the full optimizer pipeline.
    // Default: only ANSI-strip stderr so error messages always reach the AI.
    let optimize_stderr = config.as_ref().is_some_and(|c| c.optimize.optimize_stderr);

    let (raw_output, optimized) = if optimize_stderr || result.stderr.is_empty() {
        // Legacy path: combine stdout+stderr and optimize together
        let mut raw = result.stdout.clone();
        if !result.stderr.is_empty() {
            if !raw.is_empty() && !raw.ends_with(b"\n") {
                raw.push(b'\n');
            }
            raw.extend_from_slice(&result.stderr);
        }
        let opt = optimizer::run_pipeline(&raw, config.as_ref());
        (raw, opt)
    } else {
        // Optimize stdout only; ANSI-strip stderr and append unfiltered
        let opt_stdout = optimizer::run_pipeline(&result.stdout, config.as_ref());
        let stderr_clean = optimizer::strip_ansi(&String::from_utf8_lossy(&result.stderr), false);

        let mut raw = result.stdout.clone();
        if !raw.ends_with(b"\n") && !raw.is_empty() {
            raw.push(b'\n');
        }
        raw.extend_from_slice(&result.stderr);

        let mut content = opt_stdout.content;
        if !stderr_clean.is_empty() {
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(&stderr_clean);
        }

        let original_bytes = raw.len();
        let optimized_bytes = content.len();
        let opt = crate::types::OptimizedOutput {
            content,
            original_bytes,
            optimized_bytes,
            was_truncated: opt_stdout.was_truncated,
        };
        (raw, opt)
    };

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
