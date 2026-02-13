use std::fs;

use crate::storage::StorageManager;

pub fn run() {
    let storage = StorageManager::new();

    // Remove logs
    if storage.logs_dir().exists() {
        if let Err(e) = fs::remove_dir_all(storage.logs_dir()) {
            eprintln!("tkn: failed to remove logs: {e}");
        }
    }

    // Remove sessions
    if storage.sessions_dir().exists() {
        if let Err(e) = fs::remove_dir_all(storage.sessions_dir()) {
            eprintln!("tkn: failed to remove sessions: {e}");
        }
    }

    // Remove analytics
    if storage.analytics_path().exists() {
        if let Err(e) = fs::remove_file(storage.analytics_path()) {
            eprintln!("tkn: failed to remove analytics: {e}");
        }
    }

    // Recreate directory structure
    if let Err(e) = storage.init() {
        eprintln!("tkn: failed to reinitialize directories: {e}");
    }

    println!("All stats and logs cleared.");
}
