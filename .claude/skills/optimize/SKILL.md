---
name: optimize
description: Create, improve, or audit tkn optimizer plugins. Use when asked to "create a plugin for X", "optimize X output", "audit plugins", "improve the kubectl plugin", etc.
---

# Optimize: Author & Audit tkn Plugins

## Key Constraints

- Write plugins to `plugins/` (builtin), **never** `~/.tkn/tools/`
- **Never add color-related transforms** — tkn strips ANSI escape codes automatically in post-processing (`strip_ansi`), so `--no-color`, `--color=never`, etc. are unnecessary and can cause compatibility issues (e.g., Apple Git)
- The global 500-line cap exists as a safety net — plugins can override it with `max_lines`
- Always read an existing plugin TOML before modifying it
- Always use `tkn replay` to validate changes against real data

## Key Commands

- `tkn logs` — list recent runs with ref_ids, exit codes, sizes, and savings
- `tkn log <ref_id>` — show the **raw** stored output for a run
- `tkn replay <ref_id>` — re-run the **current** optimizer pipeline on a stored run (shows optimized output + before/after byte comparison)
- `tkn analyze scan` — rank all tools by optimization opportunity
- `tkn analyze report -- <tool>` — detailed stats, boilerplate detection, and output structure for a tool

## Creating a New Plugin

1. **Gather context** — Determine the target command (e.g., `kubectl get pods`). Ask if unclear.

2. **Find existing data** — Run `tkn logs` to find ref_ids for the target tool, then `tkn analyze report -- <tool>` for aggregate stats.
   - If past runs exist: use `tkn log <ref_id>` to see raw output and `tkn replay <ref_id>` to see current optimized output. Pick small/medium/large samples based on the analyze report size stats.
   - If no past runs: research the tool's output format (web search for typical output), then either run the command if it's read-only or ask the user to run it.

3. **Research transforms** — Web search for the tool's CLI flags that reduce noise (formatting, verbosity flags). Propose `[transform]` rules:
   - `add` — flags to inject (e.g., `"--short"`, `"--quiet"`)
   - `remove` — flags to strip (e.g., `"--verbose"`)
   - `replace` — flag substitutions (e.g., `{"--format=table" = "--format=json"}`)
   - **Do NOT add color flags** — ANSI stripping is handled automatically by tkn

4. **Decide optimization strategy** — Three tools available, can be combined:

   - **`strip`** (blocklist) — Remove lines matching patterns. Best when output is mostly useful with some junk lines.
   - **`keep`** (allowlist) — Only keep lines matching patterns. Best when output is mostly noise with a few useful signal lines.
   - **`replace`** (condense) — Regex substitutions applied per-line in order. Best for stripping verbose prefixes/metadata while keeping the useful part of each line (e.g., removing `ls -l` permission/owner columns, stripping timestamps).

   `strip` and `keep` are mutually exclusive (`keep` wins if both present), but **`replace` combines with either**. Common pattern: `strip` noise lines + `replace` to condense remaining lines.

   Reference `tkn analyze report` boilerplate detection and common prefixes for pattern ideas.

5. **Draft the TOML plugin** — Write to `plugins/<bundle>/<tool>.toml`:
   ```toml
   match = "<tool> <subcommand>"

   [transform]
   add = ["--quiet"]
   remove = ["--verbose"]

   [optimize]
   strip = ["^pattern_to_remove"]
   # OR
   keep = ["^pattern_to_keep"]

   # Condense verbose lines (combine with strip or keep):
   # [[optimize.replace]]
   # pattern = "^verbose_prefix\\s+"
   # replacement = ""

   # max_lines = 200  # override default 500-line cap
   # raw = false       # set true to skip blank-line collapse (for tools where whitespace matters)
   ```

6. **Register the plugin** — Add it to `builtin_plugins()` in `src/tool_config.rs` with the appropriate bundle name, plugin name, and `include_str!` path. Update the test count in `test_builtin_plugins_returns_all` and add the bundle to `test_builtin_plugins_have_bundles` if it's new.

7. **Validate with replay** — Use the edit-replay loop: after writing/editing the plugin TOML, run `tkn replay <ref_id>` on the same samples from step 2. The footer shows "Previously: X bytes → Now: Y bytes" so you can see the impact. Compare against `tkn log <ref_id>` (raw) to verify nothing important was lost.

8. **Iterate** — Use `AskUserQuestion` to present choices:
   - "Keep this version?" / "Strip more aggressively?" / "Switch from strip to keep?" / "Adjust max_lines?"
   - Use focused options, not open-ended questions

9. **Finalize** — Run `cargo test` to verify, then summarize what the plugin does.

## Auditing Existing Plugins

1. Run `tkn analyze scan` to see all tools ranked by optimization opportunity.

2. Pick the worst-performing plugin (or user-specified one).

3. Run `tkn logs` to find ref_ids for the target tool, then `tkn analyze report -- <tool>` for pattern analysis.

4. Compare raw vs optimized: `tkn log <ref_id>` for raw output, `tkn replay <ref_id>` for current optimized output on representative ref_ids.

5. Propose improvements, use the edit-replay loop to validate, iterate with the user using `AskUserQuestion`.

## Plugin TOML Reference

```toml
match = "git diff"

[transform]
add = ["--short|-s"]         # pipe-separated aliases (avoids duplicates)
remove = ["--verbose"]
replace = { "--format=table" = "--format=json" }

[optimize]
strip = ["^index [a-f0-9]+", "^diff --git"]  # regex patterns to remove lines
keep = ["^\\+", "^-", "^@@"]                  # regex patterns to keep (overrides strip)
max_lines = 200                                # override default 500-line cap (can go higher or lower)
raw = false                                    # set true to skip blank-line collapse

[[optimize.replace]]                           # ordered regex substitutions (condense lines)
pattern = "^\\d+\\s+"
replacement = ""
```

- **No color flags** — tkn strips ANSI codes automatically; never use `--no-color`, `--color=never`, etc.
- `strip` and `keep` are mutually exclusive — use one or the other (`keep` wins if both present)
- `replace` combines with either `strip` or `keep` — applied after filtering
- `raw = true` — skip blank-line collapse and trailing-whitespace trim; use for tools where whitespace/formatting is meaningful (e.g., formatted tables, tree output)
- `max_lines` overrides the global 500-line cap — use sparingly for tools that need more or less
