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

    if !analytics.tools.is_empty() {
        println!();
        println!("Per-tool usage");
        println!("{}", "-".repeat(40));

        let mut tools: Vec<_> = analytics.tools.iter().collect();
        tools.sort_by(|a, b| b.1.count.cmp(&a.1.count));

        for (tool, stats) in &tools {
            let saved = stats.total_raw_bytes.saturating_sub(stats.total_optimized_bytes);
            let pct = if stats.total_raw_bytes > 0 {
                (saved as f64 / stats.total_raw_bytes as f64) * 100.0
            } else {
                0.0
            };
            let mut extras = Vec::new();
            if stats.failures > 0 {
                let fail_rate = (stats.failures as f64 / stats.count as f64) * 100.0;
                extras.push(format!("{} failures ({:.0}%)", stats.failures, fail_rate));
            }
            if stats.full_log_reads > 0 {
                extras.push(format!("{} full reads", stats.full_log_reads));
            }
            let suffix = if extras.is_empty() {
                String::new()
            } else {
                format!("  [{}]", extras.join(", "))
            };
            println!(
                "  {:<20} {:>5}x  saved {:.0}%{}",
                tool, stats.count, pct, suffix
            );
        }
    }
}
