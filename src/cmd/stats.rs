use crate::storage::StorageManager;

pub fn run() {
    let storage = StorageManager::new();
    let analytics = storage.read_analytics();

    if analytics.total_commands == 0 {
        println!("No commands tracked yet.");
        return;
    }

    println!("tkn analytics");
    println!("{}", "-".repeat(40));
    println!("Total commands:    {}", analytics.total_commands);
    println!(
        "Total raw bytes:   {} ({:.1} KB)",
        analytics.total_raw_bytes,
        analytics.total_raw_bytes as f64 / 1024.0
    );
    println!(
        "Total optimized:   {} ({:.1} KB)",
        analytics.total_optimized_bytes,
        analytics.total_optimized_bytes as f64 / 1024.0
    );
    println!(
        "Bytes saved:       {} ({:.1}%)",
        analytics
            .total_raw_bytes
            .saturating_sub(analytics.total_optimized_bytes),
        analytics.savings_percent()
    );
    println!(
        "Avg duration:      {:.0} ms",
        analytics.total_duration_ms as f64 / analytics.total_commands as f64
    );
    if let Some(last) = analytics.last_updated {
        println!("Last updated:      {}", last.format("%Y-%m-%d %H:%M:%S UTC"));
    }
    if let Some(last) = analytics.last_cleanup {
        println!("Last cleanup:      {}", last.format("%Y-%m-%d %H:%M:%S UTC"));
    }
}
