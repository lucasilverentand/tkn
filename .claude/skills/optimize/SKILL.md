---
name: optimize
description: Create, improve, or audit tkn optimizer plugins. Use when asked to "create a plugin for X", "optimize X output", "audit plugins", "improve the kubectl plugin", etc.
---

# Optimize: Author & Audit tkn Plugins

## Key Constraints

- Write plugins to `plugins/` (builtin), **never** `~/.tkn/tools/`
- Apple Git doesn't support `--no-color` on `git status` or `git blame` — don't add it
- The global 500-line cap exists as a safety net — plugins should aim well under it
- Always read an existing plugin TOML before modifying it
- Always use `tkn replay` to validate changes against real data

## Creating a New Plugin

1. **Gather context** — Determine the target command (e.g., `kubectl get pods`). Ask if unclear.

2. **Check for existing data** — Run `tkn analyze report -- <tool>` to see if there are past runs.
   - If past runs exist: use `tkn replay <ref_id>` on representative samples (pick small/medium/large based on the analyze report stats)
   - If no past runs: research the tool's output format (web search for typical output), then either run the command if it's read-only or ask the user to run it

3. **Research transforms** — Web search for the tool's CLI flags that reduce noise (color, formatting, verbosity flags). Propose `[transform]` rules:
   - `add` — flags to inject (e.g., `"--no-color"`, `"--color=never"`)
   - `remove` — flags to strip (e.g., `"--verbose"`)
   - `replace` — flag substitutions (e.g., `{"--format=table" = "--format=json"}`)

4. **Decide strip vs keep strategy:**
   - If output is mostly noise with a few useful signal lines → use `keep` (allowlist)
   - If output is mostly useful with some junk lines → use `strip` (blocklist)
   - Reference `tkn analyze report` boilerplate detection for pattern ideas

5. **Draft the TOML plugin** — Write to `plugins/<bundle>/<tool>.toml` with this structure:
   ```toml
   match = "<tool> <subcommand>"

   [transform]
   add = ["--flag1", "--flag2"]
   remove = ["--verbose"]

   [optimize]
   strip = ["^pattern_to_remove"]
   # OR
   keep = ["^pattern_to_keep"]
   max_bytes = 16384
   ```

6. **Register the plugin** — Add it to `builtin_plugins()` in `src/tool_config.rs` with the appropriate bundle name, plugin name, and `include_str!` path. Update the test count in `test_builtin_plugins_returns_all` and add the bundle to `test_builtin_plugins_have_bundles` if it's new.

7. **Show before/after** — Use `tkn replay <ref_id>` on historical runs (or run the command) to show raw vs optimized output. Present the comparison to the user.

8. **Iterate** — Use `AskUserQuestion` to present choices:
   - "Keep this version?" / "Strip more aggressively?" / "Switch from strip to keep?" / "Adjust max_bytes?"
   - Use focused options, not open-ended questions

9. **Finalize** — Run `cargo test` to verify, then summarize what the plugin does.

## Auditing Existing Plugins

1. Run `tkn analyze scan` to see all tools ranked by optimization opportunity.

2. Pick the worst-performing plugin (or user-specified one).

3. Run `tkn analyze report -- <tool>` for pattern analysis.

4. Use `tkn replay <ref_id>` on representative ref_ids to show current behavior.

5. Propose improvements, show before/after, iterate with the user using `AskUserQuestion`.

## Plugin TOML Reference

```toml
match = "git diff"

[transform]
add = ["--no-color|--color=never"]  # pipe-separated aliases (avoids duplicates)
remove = ["--stat"]
replace = { "--format=table" = "--format=json" }

[optimize]
strip = ["^index [a-f0-9]+", "^diff --git"]  # regex patterns to remove lines
keep = ["^\\+", "^-", "^@@"]                  # regex patterns to keep (overrides strip)
max_bytes = 16384                              # truncate output beyond this
raw = false                                    # set true to skip blank-line collapse
```

- `strip` and `keep` are mutually exclusive strategies — use one or the other
- `keep` wins if both are present (lines must match a keep pattern)
- `max_bytes` triggers middle-truncation (40% head / 40% tail / 20% separator)
