use crate::storage::StorageManager;

pub fn run() {
    let storage = StorageManager::new();
    match storage.run_cleanup() {
        Ok(count) => {
            if count == 0 {
                println!("Nothing to clean up.");
            } else {
                println!("Cleaned up {count} old log entries.");
            }
        }
        Err(e) => eprintln!("tkn: cleanup failed: {e}"),
    }
}
