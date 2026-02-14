# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is tkn?

A shell proxy that intercepts CLI commands and optimizes their output to reduce token usage for AI assistants. It works as a Claude Code PreToolUse hook — commands are transparently wrapped as `tkn exec -- <original command>`, transformed via plugin rules, executed, and the output is stripped of ANSI codes, filtered, and truncated before being returned.

## Build & Test Commands

- **Build**: `cargo build`
- **Install**: `mise run install` (runs `cargo install --path . --locked`)
- **Test all**: `cargo test`
- **Test single**: `cargo test <test_name>` (e.g., `cargo test test_normalize_tool`)
- **Clippy**: `cargo clippy`

## Architecture

### Execution Pipeline (`src/cmd/exec.rs`)

1. **Transform** — apply plugin rules to the command (add/remove/replace flags)
2. **Execute** — run via `$SHELL -c <command>`, capture stdout+stderr+exit code
3. **Optimize** — strip ANSI, collapse blanks, apply regex strip/keep filters, truncate
4. **Store** — save raw log + metadata + analytics to `~/.tkn/`
5. **Display** — print optimized output + footer

### Plugin System

Plugins are TOML files in `plugins/` organized by tool bundle (e.g., `plugins/git/diff.toml`). Each defines:
- `match` — command pattern (e.g., `"git diff"`)
- `[transform]` — `add`, `remove`, `replace` rules for CLI flags
- `[optimize]` — `strip`/`keep` regex filters, `max_lines` override, `raw` toggle

**Loading priority**: user overrides in `~/.tkn/tools/*.toml` > built-in `plugins/` > `~/.tkn/settings.toml` overrides.

### Key Modules

- **`src/types.rs`** — Core types (`LogEntry`, `Analytics`, `ToolStats`) and command normalization (strips env vars, wrappers like `sudo`/`nice`, resolves subcommands via longest-prefix match)
- **`src/tool_config.rs`** — Plugin loading, config merging, pattern matching
- **`src/transformer.rs`** — Command flag transformation (alias-aware add, remove, replace)
- **`src/optimizer/`** — Output pipeline: ANSI stripping, carriage-return resolution, blank line collapse, regex filtering, smart truncation (40% head / 40% tail / 20% separator)
- **`src/runner.rs`** — Shell execution
- **`src/storage/`** — Persistence layer (`~/.tkn/logs/`, `analytics.json`, sessions, plugin manifest)
- **`src/cmd/`** — Subcommand handlers: `exec`, `hook`, `plugin`, `analyze`, `stats`, `logs`, `clean`

### Hook Integration (`src/cmd/hook.rs`)

`tkn hook install` registers in `~/.claude/settings.json` as a PreToolUse hook. `tkn hook run` reads the hook JSON from stdin, rewrites the command, and outputs a JSON response. Recursion is prevented by checking `command.starts_with("tkn ")`.

## Important Notes

- We are the author of tkn — use builtin plugins in `plugins/` only, never `~/.tkn/tools/` overrides
- User-level plugins in `~/.tkn/tools/` take priority over builtins via `load_user_config`, so stale copies there shadow builtin fixes
- Apple Git doesn't support `--no-color` on `git status` (exits 129) and has ambiguous `--no-color` on `git blame` — these flags were intentionally removed from those plugins
