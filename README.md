<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://github.com/lucasilverentand/tkn/raw/main/.github/logo-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://github.com/lucasilverentand/tkn/raw/main/.github/logo-light.svg">
    <img alt="tkn" src="https://github.com/lucasilverentand/tkn/raw/main/.github/logo-dark.svg" width="200">
  </picture>
</p>

<p align="center">
  <strong>Shell proxy that optimizes CLI output for AI assistants</strong>
</p>

<p align="center">
  <a href="https://github.com/lucasilverentand/tkn/releases"><img src="https://img.shields.io/github/v/release/lucasilverentand/tkn?style=flat-square&color=blue" alt="Release"></a>
  <a href="https://github.com/lucasilverentand/tkn/blob/main/LICENSE"><img src="https://img.shields.io/github/license/lucasilverentand/tkn?style=flat-square" alt="License"></a>
</p>

---

AI coding assistants like Claude Code spend a large chunk of their context window on raw CLI output &mdash; verbose diffs, noisy test logs, ANSI escape codes. **tkn** sits between the assistant and the shell, intercepting commands and returning optimized output that preserves the information the model actually needs while stripping everything it doesn't.

## How it works

```
Assistant runs: git diff src/
             ↓
       tkn intercepts
             ↓
  ┌─ transforms flags (e.g. adds --stat)
  ├─ captures stdout/stderr
  ├─ strips ANSI codes, filters noise
  └─ truncates to relevant lines
             ↓
   Optimized output returned
     (fewer tokens, same signal)
```

tkn uses a **plugin system** with 35+ built-in tool plugins (git, cargo, docker, kubectl, npm, and more) that know which flags to add, which output lines to keep, and how to truncate intelligently.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/lucasilverentand/tkn/main/install.sh | sh
```

This detects your OS and architecture, downloads the latest release, and installs to `/usr/local/bin`.

### From source (via Cargo)

```sh
cargo install --git https://github.com/lucasilverentand/tkn.git --locked
```

### From source (local clone)

```sh
git clone https://github.com/lucasilverentand/tkn.git
cd tkn
cargo install --path . --locked
```

## Setup

### Claude Code (automatic hook)

tkn integrates with Claude Code as a PreToolUse hook that automatically intercepts Bash commands:

```sh
tkn hook install
```

This registers tkn in `~/.claude/settings.json`. Every Bash tool call Claude makes will be routed through tkn automatically.

To remove the hook:

```sh
tkn hook uninstall
```

### Manual usage

You can also use tkn directly:

```sh
# Auto-route: tkn decides whether to optimize or pass through
tkn auto -- git diff src/

# Force optimization
tkn exec -- cargo test

# Pass through without optimization (interactive/streaming commands)
tkn pass -- docker logs -f my-container
```

## Built-in plugins

tkn ships with optimized plugins for 35+ tools:

| Category | Tools |
|----------|-------|
| **Version control** | git (diff, log, status, show, blame, branch, push, pull, fetch, stash, cherry-pick, rebase, merge, remote) |
| **Languages & build** | cargo, go, swift, xcodebuild, tsc, make |
| **Package managers** | npm, bun, pnpm, pip, deno |
| **Containers** | docker (ps, logs, build, images, inspect, compose, network, volume, system) |
| **Infrastructure** | kubectl (get, describe, logs, apply, diff, rollout, top, events, exec) |
| **Linters & formatters** | biome, eslint, prettier, ruff |
| **Search & files** | grep, rg, find, ls, tree, cat, head, tail, sed, wc, nl, du |
| **HTTP & APIs** | curl, wget, gh |
| **Testing** | pytest |

Each plugin defines flag transforms (adding `--no-pager`, removing `--color=always`, etc.) and output optimization rules (regex filters, line limits, truncation strategies).

## Commands

| Command | Description |
|---------|-------------|
| `tkn auto -- <cmd>` | Smart routing &mdash; optimize or pass through based on the command |
| `tkn exec -- <cmd>` | Force-optimize the command output |
| `tkn pass -- <cmd>` | Pass through with no optimization |
| `tkn stats` | Show token savings and usage analytics |
| `tkn log [id] [reason]` | Browse or retrieve stored command logs |
| `tkn analyze scan` | Rank tools by optimization opportunity |
| `tkn analyze report <tool>` | Analyze a specific tool's output patterns |
| `tkn plugin list` | List installed and available plugins |
| `tkn plugin install [url]` | Install built-in or third-party plugins |
| `tkn plugin remove <name>` | Remove an installed plugin |
| `tkn replay <id>` | Replay a stored command through the current optimizer |
| `tkn reasons` | Show trends in full log read reasons |
| `tkn clean` | Clear stats and logs |
| `tkn hook install` | Install the Claude Code hook |
| `tkn hook uninstall` | Remove the Claude Code hook |

## Writing plugins

Plugins are TOML files that define how to transform and optimize a tool's output:

```toml
match = "git diff"

[transform]
add = ["--no-pager", "--stat"]
remove = ["--color=always"]

[optimize]
strip = ["^\\s*$"]          # Remove blank lines
keep = ["^[+-]", "^@@"]     # Only keep diff hunks
max_lines = 500
truncate = "tail"            # Keep the end when truncating
```

Place custom plugins in `~/.tkn/tools/` to override or extend built-ins.

## License

MIT
