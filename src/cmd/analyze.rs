use std::collections::HashMap;

use crate::storage::StorageManager;
use crate::tool_config;
use crate::types::normalize_tool;

pub fn scan() {
    let storage = StorageManager::new();
    let entries = storage.list_log_entries().unwrap_or_default();

    if entries.is_empty() {
        println!("No log entries found. Run some commands through tkn first.");
        return;
    }

    let patterns = tool_config::collect_patterns();

    // Group entries by normalized tool name
    let mut tools: HashMap<String, ToolSummary> = HashMap::new();
    for entry in &entries {
        let tool = normalize_tool(&entry.command, &patterns);
        let summary = tools.entry(tool).or_default();
        summary.count += 1;
        summary.total_raw += entry.raw_bytes;
        summary.total_optimized += entry.optimized_bytes;
    }

    // Score each tool for optimization opportunity
    let mut ranked: Vec<_> = tools
        .into_iter()
        .map(|(name, summary)| {
            let has_plugin = tool_config::load_tool_config(&name).is_some();
            let savings_pct = if summary.total_raw > 0 {
                ((summary.total_raw - summary.total_optimized) as f64
                    / summary.total_raw as f64)
                    * 100.0
            } else {
                0.0
            };
            let avg_raw = summary.total_raw as f64 / summary.count as f64;

            // Opportunity score: high bytes * low savings * frequency
            // Tools with no plugin and large outputs rank highest
            let plugin_penalty = if has_plugin { 0.3 } else { 1.0 };
            let score = avg_raw * summary.count as f64 * plugin_penalty * (1.0 - savings_pct / 100.0);

            ScoredTool {
                name,
                count: summary.count,
                avg_raw,
                savings_pct,
                has_plugin,
                score,
            }
        })
        .collect();

    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

    println!("tkn analyze scan");
    println!("{}", "=".repeat(70));
    println!();
    println!(
        "  {:<22} {:>5} {:>10} {:>8} {:>8}",
        "TOOL", "RUNS", "AVG SIZE", "SAVED", "PLUGIN"
    );
    println!("  {}", "-".repeat(58));

    for tool in &ranked {
        let plugin_str = if tool.has_plugin { "yes" } else { "-" };
        println!(
            "  {:<22} {:>5} {:>8.0} B {:>6.1}% {:>8}",
            tool.name, tool.count, tool.avg_raw, tool.savings_pct, plugin_str
        );
    }

    // Suggest the top candidate
    if let Some(top) = ranked.first() {
        println!();
        if !top.has_plugin {
            println!(
                "Recommendation: \"{}\" has no plugin and {} runs averaging {:.0} bytes.",
                top.name, top.count, top.avg_raw
            );
            println!(
                "  Run: tkn analyze report -- {}",
                top.name
            );
        } else if top.savings_pct < 20.0 {
            println!(
                "Recommendation: \"{}\" has a plugin but only {:.1}% savings across {} runs.",
                top.name, top.savings_pct, top.count
            );
            println!(
                "  Run: tkn analyze report -- {}",
                top.name
            );
        } else {
            println!("All tracked tools look well-optimized.");
        }
    }
}

#[derive(Default)]
struct ToolSummary {
    count: usize,
    total_raw: usize,
    total_optimized: usize,
}

struct ScoredTool {
    name: String,
    count: usize,
    avg_raw: f64,
    savings_pct: f64,
    has_plugin: bool,
    score: f64,
}

pub fn report(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: tkn analyze report -- <command>");
        eprintln!("Example: tkn analyze report -- git branch");
        std::process::exit(1);
    }

    let command = args.join(" ");
    let patterns = tool_config::collect_patterns();
    let tool_name = normalize_tool(&command, &patterns);
    let storage = StorageManager::new();

    let entries = storage.list_log_entries().unwrap_or_default();
    let matching: Vec<_> = entries
        .iter()
        .filter(|e| normalize_tool(&e.command, &patterns) == tool_name)
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

    let mut sorted_raw = raw_sizes;
    sorted_raw.sort();

    // Print report
    println!("tkn analyze: {tool_name}");
    println!("{}", "=".repeat(60));

    println!();
    println!("Summary");
    println!("{}", "-".repeat(60));
    println!("  Samples:           {}", outputs.len());
    println!(
        "  Avg output size:   {:.0} bytes",
        total_raw as f64 / matching.len() as f64
    );
    println!(
        "  Median output:     {} bytes",
        sorted_raw[sorted_raw.len() / 2]
    );
    println!("  Min output:        {} bytes", sorted_raw[0]);
    println!("  Max output:        {} bytes", sorted_raw.last().unwrap());
    if sorted_raw.len() >= 2 {
        let p95_idx = ((sorted_raw.len() as f64 * 0.95) as usize).min(sorted_raw.len() - 1);
        println!("  P95 output:        {} bytes", sorted_raw[p95_idx]);
    }
    println!("  Current savings:   {savings_pct:.1}%");

    // Line count stats
    println!();
    println!("Line counts");
    println!("{}", "-".repeat(60));
    let mut line_counts: Vec<usize> = outputs.iter().map(|o| o.lines().count()).collect();
    line_counts.sort();
    println!(
        "  Avg lines:         {:.0}",
        line_counts.iter().sum::<usize>() as f64 / line_counts.len() as f64
    );
    println!(
        "  Median lines:      {}",
        line_counts[line_counts.len() / 2]
    );
    println!("  Min lines:         {}", line_counts[0]);
    println!("  Max lines:         {}", line_counts.last().unwrap());

    // Unique vs total lines
    let total_lines: usize = line_counts.iter().sum();
    let unique_across: usize = analysis.line_freq.len();
    println!("  Total lines (all): {total_lines}");
    println!("  Unique lines:      {unique_across}");

    if !analysis.boilerplate.is_empty() {
        println!();
        println!("Boilerplate lines (appear in >70% of samples)");
        println!("{}", "-".repeat(60));
        for (line, pct) in &analysis.boilerplate {
            let display = truncate(line, 60);
            println!("  [{pct:>3.0}%] {display}");
        }
    }

    if !analysis.common_prefixes.is_empty() {
        println!();
        println!("Repeated prefixes (>50% of samples)");
        println!("{}", "-".repeat(60));
        for (prefix, pct, avg_count) in &analysis.common_prefixes {
            println!("  [{pct:>3.0}%] \"{prefix}...\"  (~{avg_count:.0} lines/sample)");
        }
    }

    // Lines by frequency band
    println!();
    println!("Line frequency distribution");
    println!("{}", "-".repeat(60));
    let n = outputs.len() as f64;
    let high = analysis
        .line_freq
        .values()
        .filter(|&&c| (c as f64 / n) > 0.7)
        .count();
    let mid = analysis
        .line_freq
        .values()
        .filter(|&&c| {
            let pct = c as f64 / n;
            (0.2..=0.7).contains(&pct)
        })
        .count();
    let low = analysis
        .line_freq
        .values()
        .filter(|&&c| (c as f64 / n) < 0.2)
        .count();
    println!("  Stable (>70%):     {high} unique lines");
    println!("  Moderate (20-70%): {mid} unique lines");
    println!("  Volatile (<20%):   {low} unique lines");

    if !analysis.volatile.is_empty() {
        println!();
        println!("Volatile lines (appear in <20% of samples)");
        println!("{}", "-".repeat(60));
        let show = analysis.volatile.len().min(15);
        for line in &analysis.volatile[..show] {
            let display = truncate(line, 72);
            println!("  {display}");
        }
        if analysis.volatile.len() > 15 {
            println!("  ... and {} more", analysis.volatile.len() - 15);
        }
    }

    // Output structure: show a representative sample's shape
    if let Some(sample) = outputs.first() {
        println!();
        println!("Sample output structure");
        println!("{}", "-".repeat(60));
        let lines: Vec<&str> = sample.lines().collect();
        let total = lines.len();
        if total <= 20 {
            for line in &lines {
                println!("  {}", truncate(line, 72));
            }
        } else {
            for line in &lines[..8] {
                println!("  {}", truncate(line, 72));
            }
            println!("  ... ({} lines omitted) ...", total - 16);
            for line in &lines[total - 8..] {
                println!("  {}", truncate(line, 72));
            }
        }
    }

    // Current config
    println!();
    println!("Current plugin config");
    println!("{}", "-".repeat(60));
    match tool_config::load_tool_config(&command) {
        Some(config) => {
            if !config.optimize.strip.is_empty() {
                println!("  strip = {:?}", config.optimize.strip);
            }
            if !config.optimize.keep.is_empty() {
                println!("  keep = {:?}", config.optimize.keep);
            }
            if let Some(ml) = config.optimize.max_lines {
                println!("  max_lines = {ml}");
            }
            if !config.transform.add.is_empty() {
                println!("  transform.add = {:?}", config.transform.add);
            }
            if !config.transform.remove.is_empty() {
                println!("  transform.remove = {:?}", config.transform.remove);
            }
            if config.optimize.strip.is_empty()
                && config.optimize.keep.is_empty()
                && config.optimize.max_lines.is_none()
                && config.transform.add.is_empty()
                && config.transform.remove.is_empty()
            {
                println!("  (no optimization rules)");
            }
        }
        None => println!("  (no plugin config found)"),
    }
}

struct Analysis {
    /// All line frequencies for distribution stats
    line_freq: HashMap<String, usize>,
    /// Lines appearing in >70% of samples, with their percentage
    boilerplate: Vec<(String, f64)>,
    /// Prefixes appearing in >50% of samples: (prefix, pct, avg_lines_per_sample)
    common_prefixes: Vec<(String, f64, f64)>,
    /// Lines appearing in <20% of samples
    volatile: Vec<String>,
}

fn analyze(outputs: &[String]) -> Analysis {
    let n = outputs.len() as f64;
    let mut line_freq: HashMap<String, usize> = HashMap::new();
    let mut prefix_freq: HashMap<String, usize> = HashMap::new();
    // Track total occurrences (not deduplicated) for avg-per-sample
    let mut prefix_total: HashMap<String, usize> = HashMap::new();

    for output in outputs {
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
            if prefix.len() >= 5 {
                *prefix_total.entry(prefix.clone()).or_insert(0) += 1;
                if seen_prefixes.insert(prefix.clone()) {
                    *prefix_freq.entry(prefix).or_insert(0) += 1;
                }
            }
        }
    }

    let mut boilerplate: Vec<(String, f64)> = line_freq
        .iter()
        .filter(|(_, &count)| (count as f64 / n) > 0.7)
        .map(|(line, &count)| (line.clone(), (count as f64 / n) * 100.0))
        .collect();
    boilerplate.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let mut common_prefixes: Vec<(String, f64, f64)> = prefix_freq
        .iter()
        .filter(|(_, &count)| (count as f64 / n) > 0.5)
        .map(|(prefix, &count)| {
            let total = *prefix_total.get(prefix).unwrap_or(&0);
            let avg_per_sample = total as f64 / n;
            (prefix.clone(), (count as f64 / n) * 100.0, avg_per_sample)
        })
        .collect();
    common_prefixes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // Filter out prefixes that are just boilerplate line prefixes (redundant)
    let boilerplate_set: std::collections::HashSet<&str> =
        boilerplate.iter().map(|(l, _)| l.as_str()).collect();
    common_prefixes.retain(|(prefix, _, _)| {
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
        line_freq,
        boilerplate,
        common_prefixes,
        volatile,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}
