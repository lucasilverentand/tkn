use std::fs;

use crate::storage::StorageManager;

pub fn run(logs_only: bool, stats_only: bool) -> i32 {
    let storage = StorageManager::new();
    let clean_all = !logs_only && !stats_only;
    let mut failed = false;

    if clean_all || logs_only {
        if storage.logs_dir().exists() {
            if let Err(e) = fs::remove_dir_all(storage.logs_dir()) {
                eprintln!("tkn: failed to remove logs: {e}");
                failed = true;
            }
        }
        if storage.sessions_dir().exists() {
            if let Err(e) = fs::remove_dir_all(storage.sessions_dir()) {
                eprintln!("tkn: failed to remove sessions: {e}");
                failed = true;
            }
        }
    }

    if (clean_all || stats_only) && storage.analytics_path().exists() {
        if let Err(e) = fs::remove_file(storage.analytics_path()) {
            eprintln!("tkn: failed to remove analytics: {e}");
            failed = true;
        }
    }

    // Recreate directory structure
    if let Err(e) = storage.init() {
        eprintln!("tkn: failed to reinitialize directories: {e}");
        failed = true;
    }

    if clean_all {
        println!("All stats and logs cleared.");
    } else if logs_only {
        println!("Logs cleared.");
    } else {
        println!("Stats cleared.");
    }

    if failed { 1 } else { 0 }
}
