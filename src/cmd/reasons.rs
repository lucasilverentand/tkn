use std::cmp::Reverse;
use std::collections::HashMap;

use crate::storage::StorageManager;

pub fn run() {
    let storage = StorageManager::new();
    let reasons_path = storage.base_dir.join("log_reasons.jsonl");

    let content = match std::fs::read_to_string(&reasons_path) {
        Ok(c) if !c.trim().is_empty() => c,
        _ => {
            println!("No full log reads recorded yet.");
            return;
        }
    };

    let mut entries: Vec<ReasonEntry> = Vec::new();
    for line in content.lines() {
        if let Ok(entry) = serde_json::from_str::<ReasonEntry>(line) {
            entries.push(entry);
        }
    }

    if entries.is_empty() {
        println!("No full log reads recorded yet.");
        return;
    }

    // Group by tool
    let mut by_tool: HashMap<String, Vec<&ReasonEntry>> = HashMap::new();
    for entry in &entries {
        by_tool.entry(entry.tool.clone()).or_default().push(entry);
    }

    // Sort tools by count descending
    let mut sorted: Vec<_> = by_tool.into_iter().collect();
    sorted.sort_by_key(|(_, entries)| Reverse(entries.len()));

    println!("Full log reads: {} total", entries.len());
    println!("{}", "-".repeat(50));

    for (tool, tool_entries) in &sorted {
        println!();
        println!("  {} ({}x)", tool, tool_entries.len());

        // Count reasons
        let mut reason_counts: HashMap<&str, usize> = HashMap::new();
        for entry in tool_entries {
            *reason_counts.entry(&entry.reason).or_insert(0) += 1;
        }
        let mut sorted_reasons: Vec<_> = reason_counts.into_iter().collect();
        sorted_reasons.sort_by_key(|(_, count)| Reverse(*count));

        for (reason, count) in &sorted_reasons {
            if *count > 1 {
                println!("    {count}x  {reason}");
            } else {
                println!("        {reason}");
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct ReasonEntry {
    #[allow(dead_code)]
    timestamp: String,
    tool: String,
    #[allow(dead_code)]
    command: String,
    reason: String,
}
