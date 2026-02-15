use crate::storage::StorageManager;
use crate::tool_config;

pub fn run(id: Option<&str>) {
    let storage = StorageManager::new();

    match id {
        Some(ref_id) => show_log(&storage, ref_id),
        None => list_logs(&storage),
    }
}

fn show_log(storage: &StorageManager, ref_id: &str) {
    match storage.read_log_entry(ref_id) {
        Ok(entry) => {
            // Track that the full log was read (optimizer quality signal)
            let patterns = tool_config::collect_patterns();
            let _ = storage.record_full_log_read(&entry.command, &patterns);

            // Metadata is tracked but not printed — just output the raw log
        }
        Err(e) => {
            eprintln!("tkn: failed to read metadata for {ref_id}: {e}");
        }
    }

    match storage.read_log(ref_id) {
        Ok(content) => print!("{content}"),
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
