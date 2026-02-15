use std::fs;
use std::io::Read;
use std::path::PathBuf;

/// Runs as the hook itself: reads stdin JSON, rewrites the command, writes to stdout.
pub fn run() {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return;
    }

    let value: serde_json::Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => return,
    };

    let command = value
        .pointer("/tool_input/command")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Pass through if empty or already wrapped (prevent recursion)
    if command.is_empty() || command.starts_with("tkn ") {
        return;
    }

    // Complex commands (pipes, chains, subshells) run as-is — their output
    // structure is intentional and shouldn't be optimized away.
    if has_complex_syntax(command) {
        return;
    }

    // Pick subcommand: long-lived processes get `pass` (inherited stdio),
    // everything else gets `exec` (captured + optimized).
    let subcmd = if is_long_lived(command) { "pass" } else { "exec" };

    // Pass the original command verbatim via env var so it survives shell
    // arg-splitting (e.g. multi-word git commit messages keep their quoting).
    // Single-quote the value, escaping any embedded single quotes.
    let escaped = command.replace('\'', "'\\''");
    let new_command = format!("TKN_ORIGINAL_CMD='{escaped}' tkn {subcmd}");
    let response = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "updatedInput": {
                "command": new_command
            }
        }
    });

    print!("{}", serde_json::to_string(&response).unwrap());
}

pub fn install() {
    let claude_dir = match dirs::home_dir() {
        Some(h) => h.join(".claude"),
        None => {
            eprintln!("tkn: cannot determine home directory");
            return;
        }
    };

    // Update settings.json
    let settings_path = claude_dir.join("settings.json");
    let mut settings = read_settings(&settings_path);

    let hooks = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let pre_tool_use = hooks
        .as_object_mut()
        .unwrap()
        .entry("PreToolUse")
        .or_insert_with(|| serde_json::json!([]));

    let hook_entry = serde_json::json!({
        "matcher": "Bash",
        "hooks": [{
            "type": "command",
            "command": "tkn hook run"
        }]
    });

    // Check if our hook is already installed
    let arr = pre_tool_use.as_array_mut().unwrap();
    let already_installed = arr.iter().any(|entry| is_tkn_hook(entry));

    if !already_installed {
        arr.push(hook_entry);
    }

    if let Err(e) = write_settings(&settings_path, &settings) {
        eprintln!("tkn: failed to update settings: {e}");
        return;
    }

    // Clean up legacy hook script if present
    let legacy_script = claude_dir.join("hooks").join("tkn-hook.sh");
    if legacy_script.exists() {
        let _ = fs::remove_file(&legacy_script);
    }

    println!("tkn hook installed successfully.");
    println!("  Settings: {}", settings_path.display());
}

pub fn uninstall() {
    let claude_dir = match dirs::home_dir() {
        Some(h) => h.join(".claude"),
        None => {
            eprintln!("tkn: cannot determine home directory");
            return;
        }
    };

    // Clean up legacy hook script if present
    let legacy_script = claude_dir.join("hooks").join("tkn-hook.sh");
    if legacy_script.exists() {
        let _ = fs::remove_file(&legacy_script);
    }

    // Remove from settings.json
    let settings_path = claude_dir.join("settings.json");
    if settings_path.exists() {
        let mut settings = read_settings(&settings_path);

        if let Some(hooks) = settings.get_mut("hooks") {
            if let Some(pre_tool_use) = hooks.get_mut("PreToolUse") {
                if let Some(arr) = pre_tool_use.as_array_mut() {
                    arr.retain(|entry| !is_tkn_hook(entry));
                }
            }
        }

        if let Err(e) = write_settings(&settings_path, &settings) {
            eprintln!("tkn: failed to update settings: {e}");
            return;
        }
    }

    println!("tkn hook uninstalled successfully.");
}

/// Check if a hook entry is a tkn hook (matches both legacy shell script and new `tkn hook run`)
fn is_tkn_hook(entry: &serde_json::Value) -> bool {
    entry
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|hooks| {
            hooks.iter().any(|h| {
                h.get("command")
                    .and_then(|c| c.as_str())
                    .map(|c| c.contains("tkn"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn read_settings(path: &PathBuf) -> serde_json::Value {
    if path.exists() {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}))
    } else {
        serde_json::json!({})
    }
}

fn write_settings(path: &PathBuf, value: &serde_json::Value) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    fs::write(path, json)
}

/// Patterns that indicate a long-running / streaming process.
const LONG_LIVED_PREFIXES: &[&str] = &[
    "npm run dev",
    "npm start",
    "npx dev",
    "yarn dev",
    "pnpm dev",
    "bun dev",
    "bun run dev",
    "cargo watch",
    "cargo run",
    "python manage.py runserver",
    "python -m http.server",
    "flask run",
    "uvicorn",
    "gunicorn",
    "tail -f",
    "tail --follow",
    "docker compose up",
    "docker-compose up",
    "watch ",
    "ng serve",
    "next dev",
    "vite",
    "webpack serve",
];

fn is_long_lived(command: &str) -> bool {
    let cmd = command.trim();
    for pattern in LONG_LIVED_PREFIXES {
        if cmd.starts_with(pattern) {
            return true;
        }
    }
    cmd.contains(" --watch") || cmd.ends_with(" --watch")
}

/// Detects commands with pipes, logical operators, semicolons, or subshells.
/// These are intentionally complex and their output should not be optimized.
fn has_complex_syntax(command: &str) -> bool {
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = command.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\\' if !in_single => {
                chars.next(); // skip escaped char
            }
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            _ if in_single || in_double => {}
            '|' | ';' => return true,
            '&' => {
                if chars.peek() == Some(&'&') {
                    return true; // &&
                }
                return true; // background &
            }
            '(' | ')' => return true, // subshell
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipe() {
        assert!(has_complex_syntax("ls | grep foo"));
        assert!(has_complex_syntax("git log | head -20"));
    }

    #[test]
    fn test_and_chain() {
        assert!(has_complex_syntax("mkdir foo && cd foo"));
    }

    #[test]
    fn test_semicolon() {
        assert!(has_complex_syntax("ls; pwd"));
    }

    #[test]
    fn test_subshell() {
        assert!(has_complex_syntax("(cd /tmp && ls)"));
    }

    #[test]
    fn test_background() {
        assert!(has_complex_syntax("sleep 10 &"));
    }

    #[test]
    fn test_simple_commands() {
        assert!(!has_complex_syntax("git diff"));
        assert!(!has_complex_syntax("cargo test --release"));
        assert!(!has_complex_syntax("ls -la /tmp"));
    }

    #[test]
    fn test_quoted_pipes_ignored() {
        assert!(!has_complex_syntax(r#"echo "foo | bar""#));
        assert!(!has_complex_syntax("echo 'foo | bar'"));
        assert!(!has_complex_syntax(r#"grep "a && b" file.txt"#));
    }

    #[test]
    fn test_escaped_chars() {
        assert!(!has_complex_syntax(r"echo foo\|bar"));
        assert!(!has_complex_syntax(r"echo foo\;bar"));
    }

    #[test]
    fn test_complex_real_world() {
        assert!(has_complex_syntax(
            "gh issue list --state all --limit 500 --json number | jq -r '.[].number' | xargs -I {} gh issue delete {} --yes 2>&1 | tail -5"
        ));
    }
}
