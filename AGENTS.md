# AGENTS.md

Shared instructions for Codex and Claude Code in this repository.

## What is tkn?

`tkn` is a shell proxy that intercepts CLI commands and optimizes command output to reduce token usage for AI assistants.

## Build & Test Commands

- Build: `cargo build`
- Install: `mise run install` (runs `cargo install --path . --locked`)
- Test all: `cargo test`
- Test single: `cargo test <test_name>`
- Lint: `cargo clippy`

## Command Routing

- Default to `tkn auto -- <command>`.
- Use `tkn exec -- <command>` when deterministic optimized capture is explicitly desired.
- Use `tkn pass -- <command>` for long-lived, interactive, or direct-streaming commands.
- Do not wrap `tkn` with `tkn` (avoid recursion).

## Architecture

### Execution Pipeline (`src/cmd/exec.rs`)

1. Transform command arguments via plugin rules.
2. Execute via `$SHELL -c <command>` and capture stdout/stderr/exit code.
3. Optimize output (strip ANSI, filter, truncate).
4. Store raw log + metadata + analytics in `~/.tkn/`.
5. Display optimized output with a footer.

### Plugin System

Plugins are TOML files in `plugins/` by tool bundle (for example, `plugins/git/diff.toml`):

- `match`: command pattern (for example, `"git diff"`)
- `[transform]`: `add`, `remove`, `replace` flag rules
- `[optimize]`: `strip`/`keep` regex filters, `replace` rules, `max_lines`, `truncate` mode, `raw`

Loading priority:

1. User overrides in `~/.tkn/tools/*.toml`
2. Built-in `plugins/`
3. `~/.tkn/settings.toml` overrides

### Key Modules

- `src/types.rs`: core types and command normalization
- `src/tool_config.rs`: plugin loading + config merge + matching
- `src/transformer.rs`: command flag transforms
- `src/optimizer/`: output optimization pipeline
- `src/runner.rs`: shell execution
- `src/storage/`: persistence (`~/.tkn/logs/`, `analytics.json`, sessions, plugin manifest)
- `src/cmd/`: subcommand handlers (`auto`, `exec`, `pass`, `hook`, `plugin`, `analyze`, `stats`, `logs`, `clean`, `reasons`, `replay`)

### Hook Integration (`src/cmd/hook.rs`)

`tkn hook install` registers in `~/.claude/settings.json` as a PreToolUse hook. `tkn hook run` reads the hook JSON from stdin, rewrites commands to `tkn auto -- <original command>`, and outputs a JSON response.

## Important Notes

- Prefer built-in plugins in `plugins/` and do not rely on stale user overrides in `~/.tkn/tools/` when validating behavior.
- Apple Git does not reliably support `--no-color` for all subcommands; plugin defaults intentionally account for this.
