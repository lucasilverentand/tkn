use std::collections::HashMap;
use std::fs;

use crate::shell;
use crate::storage::StorageManager;
use crate::tool_config;
use crate::types::normalize_tool;

pub fn scan() -> i32 {
    let storage = StorageManager::new();
    let entries = storage.list_log_entries().unwrap_or_default();

    if entries.is_empty() {
        println!("No log entries found. Run some commands through tkn first.");
        return 0;
    }

    let patterns = tool_config::collect_patterns();
    let analytics = storage.read_analytics();

    // Group entries by normalized tool name
    let mut tools: HashMap<String, ToolSummary> = HashMap::new();
    for entry in &entries {
        let tool = normalize_tool(&entry.command, &patterns);
        let summary = tools.entry(tool).or_default();
        summary.count += 1;
        summary.total_raw += entry.raw_bytes;
        summary.total_optimized += entry.optimized_bytes;
        summary.total_duration_ms += entry.duration_ms;
        if entry.exit_code != 0 {
            summary.log_failures += 1;
        }
        if let Some(est) = entry.estimated_raw_bytes {
            summary.total_estimated_raw += est;
        }
    }

    // Score each tool for optimization opportunity
    let mut ranked: Vec<_> = tools
        .into_iter()
        .map(|(name, summary)| {
            let has_plugin = tool_config::load_tool_config(&name).is_some();
            let savings_pct = if summary.total_raw > 0 {
                ((summary.total_raw - summary.total_optimized) as f64 / summary.total_raw as f64)
                    * 100.0
            } else {
                0.0
            };

            // Use max of estimated and actual raw bytes for true pre-optimization size
            let effective_raw = summary.total_estimated_raw.max(summary.total_raw);
            let avg_raw = effective_raw as f64 / summary.count as f64;
            let avg_duration_ms = summary.total_duration_ms as f64 / summary.count as f64;

            // Merge analytics data (covers more history than log files)
            let tool_stats = analytics.tools.get(&name);
            let failures = tool_stats.map_or(summary.log_failures as u64, |s| s.failures);
            let full_log_reads = tool_stats.map_or(0, |s| s.full_log_reads);
            let analytics_count = tool_stats.map_or(summary.count as u64, |s| s.count);

            // Use the larger count for rate calculations (analytics persists longer)
            let effective_count = analytics_count.max(summary.count as u64);

            let failure_rate = if effective_count > 0 {
                failures as f64 / effective_count as f64
            } else {
                0.0
            };

            // Opportunity score: high bytes * low savings * frequency
            // Tools with no plugin and large outputs rank highest
            let plugin_penalty = if has_plugin { 0.3 } else { 1.0 };
            let mut score =
                avg_raw * summary.count as f64 * plugin_penalty * (1.0 - savings_pct / 100.0);

            // Boost score for tools where optimizer is too aggressive
            if full_log_reads > 0 {
                let read_ratio = full_log_reads as f64 / effective_count as f64;
                score *= 1.0 + read_ratio * 2.0;
            }

            // Boost score for high failure rates
            if failure_rate > 0.1 {
                score *= 1.0 + failure_rate;
            }

            ScoredTool {
                name,
                count: summary.count,
                avg_raw,
                savings_pct,
                has_plugin,
                score,
                failures,
                full_log_reads,
                avg_duration_ms,
            }
        })
        .collect();

    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    println!("tkn analyze scan");
    println!("{}", "=".repeat(80));
    println!();
    println!(
        "  {:<22} {:>5} {:>10} {:>8} {:>8} {:>8} ISSUES",
        "TOOL", "RUNS", "AVG SIZE", "SAVED", "PLUGIN", "AVG MS"
    );
    println!("  {}", "-".repeat(76));

    for tool in &ranked {
        let plugin_str = if tool.has_plugin { "yes" } else { "-" };

        let mut issues = Vec::new();
        if tool.failures > 0 {
            issues.push(format!("F:{}", tool.failures));
        }
        if tool.full_log_reads > 0 {
            issues.push(format!("R:{}", tool.full_log_reads));
        }
        let issues_str = if issues.is_empty() {
            String::new()
        } else {
            format!("[{}]", issues.join(" "))
        };

        println!(
            "  {:<22} {:>5} {:>8.0} B {:>6.1}% {:>8} {:>6.0}  {}",
            tool.name,
            tool.count,
            tool.avg_raw,
            tool.savings_pct,
            plugin_str,
            tool.avg_duration_ms,
            issues_str
        );
    }

    // Build prioritized recommendations (up to 3)
    let mut recommendations: Vec<(u8, String, String)> = Vec::new();

    // Priority 1: Full log reads (optimizer too aggressive)
    let high_reads: Vec<_> = ranked.iter().filter(|t| t.full_log_reads > 0).collect();
    if !high_reads.is_empty() {
        let names: Vec<_> = high_reads
            .iter()
            .map(|t| format!("\"{}\"", t.name))
            .collect();
        recommendations.push((
            0,
            format!(
                "Optimizer may be too aggressive for {}. Users needed full logs {} time(s).",
                names.join(", "),
                high_reads.iter().map(|t| t.full_log_reads).sum::<u64>(),
            ),
            format!("tkn analyze report -- {}", high_reads[0].name),
        ));
    }

    // Priority 2: High failure rate (>20%)
    for tool in &ranked {
        if recommendations.len() >= 3 {
            break;
        }
        let effective_count = tool.count.max(1) as f64;
        let rate = tool.failures as f64 / effective_count;
        if rate > 0.2 {
            recommendations.push((
                1,
                format!(
                    "\"{}\" has a {:.0}% failure rate ({} failures in {} runs).",
                    tool.name,
                    rate * 100.0,
                    tool.failures,
                    tool.count,
                ),
                format!("tkn analyze report -- {}", tool.name),
            ));
        }
    }

    // Priority 3: No plugin
    for tool in &ranked {
        if recommendations.len() >= 3 {
            break;
        }
        if !tool.has_plugin {
            recommendations.push((
                2,
                format!(
                    "\"{}\" has no plugin and {} runs averaging {:.0} bytes.",
                    tool.name, tool.count, tool.avg_raw,
                ),
                format!("tkn analyze report -- {}", tool.name),
            ));
        }
    }

    // Priority 4: Low savings with plugin
    for tool in &ranked {
        if recommendations.len() >= 3 {
            break;
        }
        if tool.has_plugin && tool.savings_pct < 20.0 {
            recommendations.push((
                3,
                format!(
                    "\"{}\" has a plugin but only {:.1}% savings across {} runs.",
                    tool.name, tool.savings_pct, tool.count,
                ),
                format!("tkn analyze report -- {}", tool.name),
            ));
        }
    }

    if recommendations.is_empty() {
        println!();
        println!("All tracked tools look well-optimized.");
    } else {
        println!();
        for (i, (_, msg, action)) in recommendations.iter().enumerate() {
            if i > 0 {
                println!();
            }
            println!("{}. {}", i + 1, msg);
            println!("   Run: {}", action);
        }
    }

    0
}

#[derive(Default)]
struct ToolSummary {
    count: usize,
    total_raw: usize,
    total_optimized: usize,
    total_estimated_raw: usize,
    total_duration_ms: u64,
    log_failures: usize,
}

struct ScoredTool {
    name: String,
    count: usize,
    avg_raw: f64,
    savings_pct: f64,
    has_plugin: bool,
    score: f64,
    failures: u64,
    full_log_reads: u64,
    avg_duration_ms: f64,
}

pub fn report(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("Usage: tkn analyze report -- <command>");
        eprintln!("Example: tkn analyze report -- git branch");
        return 1;
    }

    let command = shell::args_to_shell_command(args);
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
        return 1;
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
        return 1;
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

    // --- Summary ---
    println!();
    println!("Summary");
    println!("{}", "-".repeat(60));
    println!("  Samples:           {}", outputs.len());
    let avg_output = total_raw as f64 / matching.len() as f64;
    println!("  Avg output size:   {avg_output:.0} bytes");
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

    // Estimated savings from transforms (savings_factor data)
    let estimated_entries: Vec<_> = matching
        .iter()
        .filter_map(|e| e.estimated_raw_bytes.map(|est| (est, e.raw_bytes)))
        .collect();
    if !estimated_entries.is_empty() {
        let total_estimated: usize = estimated_entries.iter().map(|(est, _)| est).sum();
        let total_actual: usize = estimated_entries.iter().map(|(_, raw)| raw).sum();
        let avg_estimated = total_estimated as f64 / estimated_entries.len() as f64;
        let avg_actual = total_actual as f64 / estimated_entries.len() as f64;
        let transform_savings = if total_estimated > 0 {
            ((total_estimated - total_actual) as f64 / total_estimated as f64) * 100.0
        } else {
            0.0
        };
        println!();
        println!("  Estimated pre-transform avg: {avg_estimated:.0} bytes");
        println!("  Actual avg (post-transform): {avg_actual:.0} bytes");
        println!("  Transform savings:           {transform_savings:.1}%");
    }

    // --- Reliability ---
    let failed: Vec<_> = matching.iter().filter(|e| e.exit_code != 0).collect();
    if !failed.is_empty() {
        let failure_rate = (failed.len() as f64 / matching.len() as f64) * 100.0;
        println!();
        println!("Reliability");
        println!("{}", "-".repeat(60));
        println!(
            "  Runs: {}  Failures: {}  ({failure_rate:.1}%)",
            matching.len(),
            failed.len()
        );
        let show = failed.len().min(5);
        println!("  Recent failures:");
        for entry in failed.iter().rev().take(show) {
            let cmd = truncate(&entry.command, 50);
            println!("    exit {} | {cmd}", entry.exit_code);
        }
    }

    // --- Performance ---
    let durations: Vec<u64> = matching.iter().map(|e| e.duration_ms).collect();
    let any_nonzero = durations.iter().any(|&d| d > 0);
    if any_nonzero {
        let mut sorted_dur = durations;
        sorted_dur.sort();
        let sum: u64 = sorted_dur.iter().sum();
        let avg = sum as f64 / sorted_dur.len() as f64;
        let median = sorted_dur[sorted_dur.len() / 2];
        let min = sorted_dur[0];
        let max = *sorted_dur.last().unwrap();

        println!();
        println!("Performance");
        println!("{}", "-".repeat(60));
        println!("  Avg duration:      {avg:.0} ms");
        println!("  Median duration:   {median} ms");
        println!("  Min duration:      {min} ms");
        println!("  Max duration:      {max} ms");
        if sorted_dur.len() >= 2 {
            let p95_idx = ((sorted_dur.len() as f64 * 0.95) as usize).min(sorted_dur.len() - 1);
            println!("  P95 duration:      {} ms", sorted_dur[p95_idx]);
        }
    }

    // --- Transformations ---
    let transformed: Vec<_> = matching
        .iter()
        .filter_map(|e| e.transformed_command.as_ref().map(|tc| (&e.command, tc)))
        .collect();
    if !transformed.is_empty() {
        println!();
        println!("Transformations");
        println!("{}", "-".repeat(60));
        println!(
            "  {}/{} runs had transforms applied",
            transformed.len(),
            matching.len()
        );

        // Show unique transform pairs (dedup by transformed command)
        let mut seen = std::collections::HashSet::new();
        let mut examples: Vec<(&str, &str)> = Vec::new();
        for (orig, tc) in &transformed {
            if seen.insert(tc.as_str()) {
                examples.push((orig, tc));
            }
            if examples.len() >= 3 {
                break;
            }
        }
        if !examples.is_empty() {
            println!("  Examples:");
            for (orig, tc) in &examples {
                println!("    {} -> {}", truncate(orig, 35), truncate(tc, 35));
            }
        }
    }

    // --- Optimizer Feedback (full log read reasons) ---
    let reasons = load_log_reasons(&storage, &tool_name);
    if !reasons.is_empty() {
        println!();
        println!("Optimizer Feedback");
        println!("{}", "-".repeat(60));
        println!("  Full log reads:    {}", reasons.len());
        println!("  Recent reasons:");
        let show = reasons.len().min(5);
        for reason in reasons.iter().rev().take(show) {
            println!("    - {}", truncate(reason, 68));
        }
    }

    // --- Line count stats ---
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

    // --- Current plugin config ---
    println!();
    println!("Current plugin config");
    println!("{}", "-".repeat(60));
    match tool_config::load_tool_config(&command) {
        Some(config) => {
            let mut has_rules = false;
            if !config.optimize.strip.is_empty() {
                println!("  strip = {:?}", config.optimize.strip);
                has_rules = true;
            }
            if !config.optimize.keep.is_empty() {
                println!("  keep = {:?}", config.optimize.keep);
                has_rules = true;
            }
            if !config.optimize.replace.is_empty() {
                for rule in &config.optimize.replace {
                    if rule.replacement.is_empty() {
                        println!("  replace: /{}/  (delete)", rule.pattern);
                    } else {
                        println!("  replace: /{}/ -> \"{}\"", rule.pattern, rule.replacement);
                    }
                }
                has_rules = true;
            }
            if let Some(ml) = config.optimize.max_lines {
                println!("  max_lines = {ml}");
                has_rules = true;
            }
            if !matches!(config.optimize.truncate, tool_config::TruncateMode::Middle) {
                println!("  truncate = {:?}", config.optimize.truncate);
                has_rules = true;
            }
            if !config.transform.add.is_empty() {
                println!("  transform.add = {:?}", config.transform.add);
                has_rules = true;
            }
            if !config.transform.remove.is_empty() {
                println!("  transform.remove = {:?}", config.transform.remove);
                has_rules = true;
            }
            if !config.transform.replace.is_empty() {
                println!("  transform.replace = {:?}", config.transform.replace);
                has_rules = true;
            }
            if let Some(sf) = config.transform.savings_factor {
                println!("  transform.savings_factor = {sf}");
                has_rules = true;
            }
            if !has_rules {
                println!("  (no optimization rules)");
            }
        }
        None => println!("  (no plugin config found)"),
    }

    0
}

/// Read log_reasons.jsonl and return reasons matching the given tool name.
fn load_log_reasons(storage: &StorageManager, tool_name: &str) -> Vec<String> {
    let reasons_path = storage.base_dir.join("log_reasons.jsonl");
    let content = match fs::read_to_string(&reasons_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    content
        .lines()
        .filter_map(|line| {
            let val: serde_json::Value = serde_json::from_str(line).ok()?;
            let tool = val.get("tool")?.as_str()?;
            if tool == tool_name {
                val.get("reason")?.as_str().map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect()
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
        let mut seen_prefixes: std::collections::HashSet<String> = std::collections::HashSet::new();

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
