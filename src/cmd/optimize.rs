use std::collections::HashMap;

use crate::storage::StorageManager;
use crate::tool_config;
use crate::types::normalize_tool;

pub fn run(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: tkn optimize -- <command>");
        eprintln!("Example: tkn optimize -- git branch");
        std::process::exit(1);
    }

    let command = args.join(" ");
    let tool_name = normalize_tool(&command);
    let storage = StorageManager::new();

    let entries = storage.list_log_entries().unwrap_or_default();
    let matching: Vec<_> = entries
        .iter()
        .filter(|e| normalize_tool(&e.command) == tool_name)
        .collect();

    if matching.is_empty() {
        eprintln!("No log entries found for tool: {tool_name}");
        eprintln!("Run some commands through tkn first:");
        eprintln!("  tkn exec -- {command}");
        std::process::exit(1);
    }

    // Read all raw outputs
    let mut outputs: Vec<String> = Vec::new();
    for entry in &matching {
        if let Ok(content) = storage.read_log(&entry.ref_id) {
            outputs.push(content);
        }
    }

    if outputs.is_empty() {
        eprintln!("Could not read any log files for tool: {tool_name}");
        std::process::exit(1);
    }

    let analysis = analyze(&outputs);

    // Size stats from log entries
    let raw_sizes: Vec<usize> = matching.iter().map(|e| e.raw_bytes).collect();
    let opt_sizes: Vec<usize> = matching.iter().map(|e| e.optimized_bytes).collect();
    let total_raw: usize = raw_sizes.iter().sum();
    let total_opt: usize = opt_sizes.iter().sum();
    let savings_pct = if total_raw > 0 {
        ((total_raw - total_opt) as f64 / total_raw as f64) * 100.0
    } else {
        0.0
    };

    let mut sorted_raw = raw_sizes.clone();
    sorted_raw.sort();

    // Print report
    println!("tkn optimize: {tool_name}");
    println!("{}", "=".repeat(50));

    println!();
    println!("Summary");
    println!("{}", "-".repeat(50));
    println!("  Samples:           {}", outputs.len());
    println!(
        "  Avg output size:   {:.0} bytes",
        total_raw as f64 / matching.len() as f64
    );
    println!(
        "  Median output:     {} bytes",
        sorted_raw[sorted_raw.len() / 2]
    );
    println!("  Max output:        {} bytes", sorted_raw.last().unwrap());
    println!("  Current savings:   {savings_pct:.1}%");

    if !analysis.boilerplate.is_empty() {
        println!();
        println!("Boilerplate lines (>70% of samples)");
        println!("{}", "-".repeat(50));
        for (line, pct) in &analysis.boilerplate {
            let display = truncate(line, 60);
            println!("  [{pct:>3.0}%] {display}");
        }
    }

    if !analysis.common_prefixes.is_empty() {
        println!();
        println!("Common prefixes (>50% of samples)");
        println!("{}", "-".repeat(50));
        for (prefix, pct) in &analysis.common_prefixes {
            println!("  [{pct:>3.0}%] \"{prefix}...\"");
        }
    }

    if !analysis.volatile.is_empty() {
        println!();
        println!("Volatile lines (<20% of samples, likely useful signal)");
        println!("{}", "-".repeat(50));
        let show = analysis.volatile.len().min(10);
        for line in &analysis.volatile[..show] {
            let display = truncate(line, 70);
            println!("  {display}");
        }
        if analysis.volatile.len() > 10 {
            println!("  ... and {} more", analysis.volatile.len() - 10);
        }
    }

    // Current config
    println!();
    println!("Current config");
    println!("{}", "-".repeat(50));
    match tool_config::load_tool_config(&command) {
        Some(config) => {
            if !config.optimize.strip.is_empty() {
                println!("  strip = {:?}", config.optimize.strip);
            }
            if !config.optimize.keep.is_empty() {
                println!("  keep = {:?}", config.optimize.keep);
            }
            if let Some(mb) = config.optimize.max_bytes {
                println!("  max_bytes = {mb}");
            }
            if config.optimize.strip.is_empty()
                && config.optimize.keep.is_empty()
                && config.optimize.max_bytes.is_none()
            {
                println!("  (no optimization rules)");
            }
        }
        None => println!("  (no plugin config found)"),
    }

    // Suggested TOML
    println!();
    println!("Suggested TOML");
    println!("{}", "-".repeat(50));
    print_suggested_toml(&tool_name, &analysis, &sorted_raw);
}

struct Analysis {
    /// Lines appearing in >70% of samples, with their percentage
    boilerplate: Vec<(String, f64)>,
    /// Prefixes appearing in >50% of samples, with percentage
    common_prefixes: Vec<(String, f64)>,
    /// Lines appearing in <20% of samples
    volatile: Vec<String>,
}

fn analyze(outputs: &[String]) -> Analysis {
    let n = outputs.len() as f64;
    let mut line_freq: HashMap<String, usize> = HashMap::new();
    let mut prefix_freq: HashMap<String, usize> = HashMap::new();

    for output in outputs {
        // Track unique lines per output (don't double-count within one output)
        let mut seen_lines: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut seen_prefixes: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if seen_lines.insert(trimmed.to_string()) {
                *line_freq.entry(trimmed.to_string()).or_insert(0) += 1;
            }

            let prefix: String = trimmed.chars().take(20).collect();
            if prefix.len() >= 5 && seen_prefixes.insert(prefix.clone()) {
                *prefix_freq.entry(prefix).or_insert(0) += 1;
            }
        }
    }

    let mut boilerplate: Vec<(String, f64)> = line_freq
        .iter()
        .filter(|(_, &count)| (count as f64 / n) > 0.7)
        .map(|(line, &count)| (line.clone(), (count as f64 / n) * 100.0))
        .collect();
    boilerplate.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let mut common_prefixes: Vec<(String, f64)> = prefix_freq
        .iter()
        .filter(|(_, &count)| (count as f64 / n) > 0.5)
        .map(|(prefix, &count)| (prefix.clone(), (count as f64 / n) * 100.0))
        .collect();
    common_prefixes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // Filter out prefixes that are just boilerplate line prefixes (redundant)
    let boilerplate_set: std::collections::HashSet<&str> =
        boilerplate.iter().map(|(l, _)| l.as_str()).collect();
    common_prefixes.retain(|(prefix, _)| {
        !boilerplate_set
            .iter()
            .any(|bl| bl.starts_with(prefix.as_str()))
    });

    let volatile: Vec<String> = line_freq
        .iter()
        .filter(|(_, &count)| (count as f64 / n) < 0.2)
        .map(|(line, _)| line.clone())
        .collect();

    Analysis {
        boilerplate,
        common_prefixes,
        volatile,
    }
}

fn print_suggested_toml(tool_name: &str, analysis: &Analysis, sorted_sizes: &[usize]) {
    println!("match = \"{tool_name}\"");
    println!();
    println!("[optimize]");

    if !analysis.boilerplate.is_empty() {
        println!("strip = [");
        for (line, _) in &analysis.boilerplate {
            let escaped = regex_escape(line);
            println!("    \"^{escaped}$\",");
        }
        println!("]");
    }

    // Suggest max_bytes based on p95 of observed sizes
    if sorted_sizes.len() >= 2 {
        let p95_idx = (sorted_sizes.len() as f64 * 0.95) as usize;
        let p95 = sorted_sizes[p95_idx.min(sorted_sizes.len() - 1)];
        // Round up to nearest power of 2 KB
        let suggested = next_power_of_two_kb(p95);
        println!("# max_bytes = {suggested}  # based on p95 of observed outputs");
    }
}

fn regex_escape(s: &str) -> String {
    let special = [
        '\\', '.', '+', '*', '?', '(', ')', '[', ']', '{', '}', '|', '^', '$',
    ];
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if special.contains(&c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn next_power_of_two_kb(bytes: usize) -> usize {
    let kb = bytes.div_ceil(1024);
    let power = (kb as f64).log2().ceil() as u32;
    (1 << power) * 1024
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}
