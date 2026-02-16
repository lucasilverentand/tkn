use std::collections::BTreeMap;

use crate::storage::StorageManager;
use crate::tool_config;
use crate::types::ToolStats;

pub fn run(reset: Option<&str>) {
    let storage = StorageManager::new();

    if let Some(tool) = reset {
        match storage.reset_tool_stats(tool) {
            Ok(true) => println!("Reset stats for \"{tool}\"."),
            Ok(false) => eprintln!("tkn: no stats found for \"{tool}\""),
            Err(e) => eprintln!("tkn: failed to reset stats: {e}"),
        }
        return;
    }

    let analytics = storage.read_analytics();

    if analytics.total_commands == 0 {
        println!("No commands tracked yet.");
        return;
    }

    println!("tkn analytics");
    println!("{}", "-".repeat(40));
    println!("Total commands:   {}", analytics.total_commands);
    println!(
        "Total raw bytes:  {} ({:.1} KB)",
        analytics.total_raw_bytes,
        analytics.total_raw_bytes as f64 / 1024.0
    );
    println!(
        "Total optimized:  {} ({:.1} KB)",
        analytics.total_optimized_bytes,
        analytics.total_optimized_bytes as f64 / 1024.0
    );
    let saved = analytics
        .total_raw_bytes
        .saturating_sub(analytics.total_optimized_bytes);
    println!(
        "Bytes saved:      {} ({:.1}%)",
        saved,
        analytics.savings_percent()
    );
    if analytics.total_estimated_raw_bytes > analytics.total_raw_bytes {
        let est_saved = analytics
            .total_estimated_raw_bytes
            .saturating_sub(analytics.total_optimized_bytes);
        println!(
            "Est. total saved: {} ({:.1}%) incl. transform savings",
            est_saved,
            analytics.estimated_savings_percent()
        );
    }
    println!(
        "Avg duration:     {:.0} ms",
        analytics.total_duration_ms as f64 / analytics.total_commands as f64
    );
    if let Some(last) = analytics.last_updated {
        println!(
            "Last updated:     {}",
            last.format("%Y-%m-%d %H:%M:%S UTC")
        );
    }
    if let Some(last) = analytics.last_cleanup {
        println!(
            "Last cleanup:     {}",
            last.format("%Y-%m-%d %H:%M:%S UTC")
        );
    }

    if !analytics.tools.is_empty() {
        println!();
        println!("Per-tool usage");
        println!("{}", "-".repeat(40));

        let patterns = tool_config::collect_patterns();

        // Group tools by main command (first word)
        let mut groups: BTreeMap<String, Vec<(&str, &ToolStats)>> = BTreeMap::new();
        for (tool, stats) in &analytics.tools {
            let main_cmd = tool.split_whitespace().next().unwrap_or(tool);
            groups
                .entry(main_cmd.to_string())
                .or_default()
                .push((tool, stats));
        }

        // Sort entries within each group by count descending
        for entries in groups.values_mut() {
            entries.sort_by(|a, b| b.1.count.cmp(&a.1.count));
        }

        // Sort groups by total count descending
        let mut sorted_groups: Vec<_> = groups.into_iter().collect();
        sorted_groups.sort_by(|a, b| {
            let total_a: u64 = a.1.iter().map(|(_, s)| s.count).sum();
            let total_b: u64 = b.1.iter().map(|(_, s)| s.count).sum();
            total_b.cmp(&total_a)
        });

        for (main_cmd, entries) in &sorted_groups {
            let group_count: u64 = entries.iter().map(|(_, s)| s.count).sum();
            let group_raw: u64 = entries.iter().map(|(_, s)| s.total_raw_bytes).sum();
            let group_opt: u64 = entries.iter().map(|(_, s)| s.total_optimized_bytes).sum();
            let group_saved = group_raw.saturating_sub(group_opt);
            let group_pct = if group_raw > 0 {
                (group_saved as f64 / group_raw as f64) * 100.0
            } else {
                0.0
            };

            if entries.len() == 1 {
                // Single entry — print flat (no group header)
                let (tool, stats) = &entries[0];
                print_tool_line(tool, stats, &patterns, "  ", 30);
            } else {
                // Group header
                let group_est: u64 = entries.iter().map(|(_, s)| s.total_estimated_raw_bytes).sum();
                let group_suffix = if group_est > group_raw {
                    let est_saved = group_est.saturating_sub(group_opt);
                    let est_pct = if group_est > 0 {
                        (est_saved as f64 / group_est as f64) * 100.0
                    } else {
                        0.0
                    };
                    format!("  (~{:.0}% incl. transforms)", est_pct)
                } else {
                    String::new()
                };
                println!("  {:<30} {:>5}x  saved {:.0}%{}", main_cmd, group_count, group_pct, group_suffix);
                for (tool, stats) in entries {
                    print_tool_line(tool, stats, &patterns, "    ", 28);
                }
            }
        }
    }
}

fn print_tool_line(
    tool: &str,
    stats: &ToolStats,
    patterns: &std::collections::HashSet<String>,
    indent: &str,
    width: usize,
) {
    let has_plugin = patterns.contains(tool);
    let marker = if has_plugin { " ✓" } else { "" };
    // Reserve space for marker when truncating
    let max_name = if has_plugin { width - 2 } else { width };
    let truncated = if tool.len() > max_name {
        &tool[..max_name]
    } else {
        tool
    };
    let label = format!("{truncated}{marker}");
    // ✓ is multi-byte but 1 display column; pad manually
    let display_len = truncated.len() + if has_plugin { 2 } else { 0 };
    let padding = if display_len < width {
        width - display_len
    } else {
        0
    };

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
    if stats.transformations > 0 {
        extras.push(format!("{} transforms", stats.transformations));
    }
    if stats.total_estimated_raw_bytes > stats.total_raw_bytes {
        let est_pct = if stats.total_estimated_raw_bytes > 0 {
            let est_saved = stats
                .total_estimated_raw_bytes
                .saturating_sub(stats.total_optimized_bytes);
            (est_saved as f64 / stats.total_estimated_raw_bytes as f64) * 100.0
        } else {
            0.0
        };
        extras.push(format!("~{:.0}% incl. transforms", est_pct));
    }
    let suffix = if extras.is_empty() {
        String::new()
    } else {
        format!("  [{}]", extras.join(", "))
    };
    println!(
        "{indent}{label}{:padding$} {:>5}x  saved {:.0}%{suffix}",
        "", stats.count, pct,
    );
}
