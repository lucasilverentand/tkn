use crate::optimizer;
use crate::storage::StorageManager;
use crate::tool_config;

pub fn run(ref_id: &str) -> i32 {
    let storage = StorageManager::new();

    let entry = match storage.read_log_entry(ref_id) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("tkn: failed to read metadata for {ref_id}: {e}");
            return 1;
        }
    };

    let raw = match storage.read_log(ref_id) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("tkn: failed to read raw log for {ref_id}: {e}");
            return 1;
        }
    };

    // Load current plugin config for this command
    let patterns = tool_config::collect_patterns();
    let config = tool_config::load_tool_config_with_patterns(&entry.command, &patterns);

    // Run through the current optimizer pipeline
    let result = optimizer::run_pipeline(raw.as_bytes(), config.as_ref());

    // Header
    println!("Ref:     {}", entry.ref_id);
    println!("Command: {}", entry.command);
    println!("Exit:    {}", entry.exit_code);
    println!("{}", "-".repeat(40));

    // Optimized output
    print!("{}", result.content);
    if !result.content.ends_with('\n') {
        println!();
    }

    // Footer with comparison
    println!("{}", "-".repeat(40));
    println!(
        "Raw: {} bytes → Optimized: {} bytes (saved {:.0}%)",
        result.original_bytes,
        result.optimized_bytes,
        if result.original_bytes > 0 {
            let saved = result.original_bytes.saturating_sub(result.optimized_bytes);
            (saved as f64 / result.original_bytes as f64) * 100.0
        } else {
            0.0
        }
    );

    // Compare against original optimization
    if result.optimized_bytes != entry.optimized_bytes {
        println!(
            "Previously: {} bytes → Now: {} bytes",
            entry.optimized_bytes, result.optimized_bytes
        );
    } else {
        println!("No change from original optimization.");
    }

    if result.was_truncated {
        println!("(output was truncated)");
    }

    0
}
