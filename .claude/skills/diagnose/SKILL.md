---
name: diagnose
description: Analyze tkn log reasons and analytics to find why LLMs read full logs, then fix the underlying plugin issues. Use when asked to "diagnose", "reduce full log reads", "why are logs being read", "analyze reasons", etc.
---

# Diagnose: Analyze Full-Log Reads & Fix Root Causes

Analyze why LLMs needed to bypass tkn's optimization and read full unoptimized output, then fix the plugins causing it.

## Step 1 — Gather Data

Run these in parallel:

- `cat ~/.tkn/log_reasons.jsonl` — every full-log read with tool, command, reason, and timestamp
- Analyze `~/.tkn/analytics.json` — extract tools with `full_log_reads > 0`, sort by read count, compute read-to-execution ratio

## Step 2 — Categorize Reasons

Group the reasons by tool and identify root-cause patterns:

| Pattern | Description | Example |
|---------|-------------|---------|
| **over-truncation** | `max_lines` too low for the tool's typical use | `nl` truncating file reads at 300 lines |
| **critical data stripped** | strip rule removes data the LLM actually needs | `gh pr` stripping GitHub URLs |
| **wrong optimization mode** | tool output shouldn't be optimized at all | file-reading commands like `nl`, `cat` with `raw = true` needed |
| **insufficient max_lines for structured output** | JSON/structured queries truncated | `gh pr view --json` at 100 lines |

## Step 3 — Rank by Impact

Present a summary table to the user, sorted by number of full-log reads:

```
Tool            Reads  Total Runs  Ratio   Root Cause
nl                 25           9  277.8%  over-truncation / should be raw
gh pr              12         270    4.4%  URL stripped + JSON truncated
git diff            2        1296    0.2%  occasional large diffs
...
```

For each tool, recommend one of:
1. **Set `raw = true`** — for tools where output IS the content (file readers)
2. **Remove strip rule** — when a rule removes critical data
3. **Increase `max_lines`** — when truncation is too aggressive
4. **Add `keep` patterns** — when most output is noise but some lines are critical
5. **Skip** — when read count is negligible and the current config is reasonable

## Step 4 — Present Options

Use `AskUserQuestion` to present each tool's issue and proposed fix as a numbered list. Let the user approve/reject/modify each one. Example:

```
1. nl (25 reads): set raw = true — file content shouldn't be truncated
2. gh pr (12 reads): remove URL strip rule + bump max_lines to 200
3. git diff (2 reads): keep as-is, 300 lines is reasonable for most diffs
```

## Step 5 — Apply Fixes

For each approved fix:

1. Read the current plugin TOML in `plugins/<bundle>/<tool>.toml`
2. Apply the change
3. If past log data exists, run `tkn replay <ref_id>` on a sample to verify the fix helps
4. Update tests in `tests/plugins.rs` if strip/keep rules changed

## Step 6 — Validate

Run `cargo test` to ensure all tests pass after changes.

## Key Files

- `~/.tkn/log_reasons.jsonl` — raw reason entries (tool, command, reason, timestamp)
- `~/.tkn/analytics.json` — aggregate stats per tool including `full_log_reads`
- `plugins/` — builtin plugin TOMLs
- `tests/plugins.rs` — plugin integration tests
