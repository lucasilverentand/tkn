use std::cmp::Reverse;
use std::collections::BTreeMap;

use crate::storage::StorageManager;
use crate::tool_config;
use crate::types::ToolStats;

pub fn run(reset: Option<&str>, reset_failures: Option<&str>) -> i32 {
    let storage = StorageManager::new();

    if let Some(tool) = reset {
        return match storage.reset_tool_stats(tool) {
            Ok(true) => {
                println!("Reset stats for \"{tool}\".");
                0
            }
            Ok(false) => {
                eprintln!("tkn: no stats found for \"{tool}\"");
                1
            }
            Err(e) => {
                eprintln!("tkn: failed to reset stats: {e}");
                1
            }
        };
    }

    if let Some(tool) = reset_failures {
        return match storage.reset_tool_failures(tool) {
            Ok(true) => {
                println!("Reset failures for \"{tool}\".");
                0
            }
            Ok(false) => {
                eprintln!("tkn: no stats found for \"{tool}\"");
                1
            }
            Err(e) => {
                eprintln!("tkn: failed to reset failures: {e}");
                1
            }
        };
    }

    let analytics = storage.read_analytics();

    if analytics.total_commands == 0 {
        println!("No commands tracked yet.");
        return 0;
    }

    println!("tkn analytics");
    println!("{}", "-".repeat(40));
    println!("Total commands:   {}", analytics.total_commands);
    let effective_raw = analytics
        .total_estimated_raw_bytes
        .max(analytics.total_raw_bytes);
    println!(
        "Total raw bytes:  {} ({:.1} KB)",
        effective_raw,
        effective_raw as f64 / 1024.0
    );
    println!(
        "Total optimized:  {} ({:.1} KB)",
        analytics.total_optimized_bytes,
        analytics.total_optimized_bytes as f64 / 1024.0
    );
    let saved = effective_raw.saturating_sub(analytics.total_optimized_bytes);
    let pct = if effective_raw > 0 {
        (saved as f64 / effective_raw as f64) * 100.0
    } else {
        0.0
    };
    println!("Bytes saved:      {} ({:.1}%)", saved, pct);
    println!(
        "Avg duration:     {:.0} ms",
        analytics.total_duration_ms as f64 / analytics.total_commands as f64
    );
    if let Some(last) = analytics.last_updated {
        println!("Last updated:     {}", last.format("%Y-%m-%d %H:%M:%S UTC"));
    }
    if let Some(last) = analytics.last_cleanup {
        println!("Last cleanup:     {}", last.format("%Y-%m-%d %H:%M:%S UTC"));
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
            entries.sort_by_key(|(_, stats)| Reverse(stats.count));
        }

        // Sort groups by total count descending
        let mut sorted_groups: Vec<_> = groups.into_iter().collect();
        sorted_groups.sort_by(|a, b| {
            let total_a: u64 = a.1.iter().map(|(_, s)| s.count).sum();
            let total_b: u64 = b.1.iter().map(|(_, s)| s.count).sum();
            total_b.cmp(&total_a)
        });

        // Compute the column width needed to align all entries.
        // Each line has: indent + name + marker + gap -> count column
        // We want all count columns to align at the same position.
        let mut max_col: usize = 0;
        for (main_cmd, entries) in &sorted_groups {
            if entries.len() == 1 {
                let (tool, _) = &entries[0];
                let marker_w = if patterns.contains(*tool) { 2 } else { 0 };
                // indent=2 + name + marker
                max_col = max_col.max(2 + tool.len() + marker_w);
            } else {
                // Group header: indent=2 + main_cmd
                max_col = max_col.max(2 + main_cmd.len());
                for (tool, _) in entries {
                    let marker_w = if patterns.contains(*tool) { 2 } else { 0 };
                    // indent=4 + name + marker
                    max_col = max_col.max(4 + tool.len() + marker_w);
                }
            }
        }
        // Add minimum gap before count column
        let col = max_col + 2;

        for (main_cmd, entries) in &sorted_groups {
            let group_count: u64 = entries.iter().map(|(_, s)| s.count).sum();
            let group_raw: u64 = entries.iter().map(|(_, s)| s.total_raw_bytes).sum();
            let group_est: u64 = entries
                .iter()
                .map(|(_, s)| s.total_estimated_raw_bytes)
                .sum();
            let group_effective = group_est.max(group_raw);
            let group_opt: u64 = entries.iter().map(|(_, s)| s.total_optimized_bytes).sum();
            let group_pct = if group_effective > 0 {
                let group_saved = group_effective.saturating_sub(group_opt);
                (group_saved as f64 / group_effective as f64) * 100.0
            } else {
                0.0
            };

            if entries.len() == 1 {
                // Single entry — print flat (no group header)
                let (tool, stats) = &entries[0];
                let width = col - 2; // indent=2
                print_tool_line(tool, stats, &patterns, "  ", width);
            } else {
                // Group header
                let header_width = col - 2; // indent=2
                println!(
                    "  {:<header_width$} {:>5}x  saved {:.0}%",
                    main_cmd, group_count, group_pct
                );
                let sub_width = col - 4; // indent=4
                for (tool, stats) in entries {
                    print_tool_line(tool, stats, &patterns, "    ", sub_width);
                }
            }
        }
    }

    0
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
    let padding = width.saturating_sub(display_len);

    let effective_raw = stats.total_estimated_raw_bytes.max(stats.total_raw_bytes);
    let pct = if effective_raw > 0 {
        let saved = effective_raw.saturating_sub(stats.total_optimized_bytes);
        (saved as f64 / effective_raw as f64) * 100.0
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
