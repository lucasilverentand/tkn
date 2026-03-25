use std::fs;
use std::io::Read;
use std::path::PathBuf;

use super::routing;

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

    if routing::should_skip(command) {
        return;
    }

    // Pass the original command as a single shell-quoted argument so it
    // survives arg-splitting (e.g. multi-word git commit messages keep their
    // quoting). Single-quote the value, escaping any embedded single quotes.
    let escaped = command.replace('\'', "'\\''");
    let new_command = format!("tkn auto -- '{escaped}'");
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

pub fn install() -> i32 {
    let claude_dir = match dirs::home_dir() {
        Some(h) => h.join(".claude"),
        None => {
            eprintln!("tkn: cannot determine home directory");
            return 1;
        }
    };

    // Update settings.json
    let settings_path = claude_dir.join("settings.json");
    let mut settings = read_settings(&settings_path);

    let Some(settings_obj) = settings.as_object_mut() else {
        eprintln!("tkn: settings.json is not a JSON object");
        return 1;
    };

    let hooks = settings_obj
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let Some(hooks_obj) = hooks.as_object_mut() else {
        eprintln!("tkn: settings.json 'hooks' is not an object");
        return 1;
    };

    let pre_tool_use = hooks_obj
        .entry("PreToolUse")
        .or_insert_with(|| serde_json::json!([]));

    let Some(arr) = pre_tool_use.as_array_mut() else {
        eprintln!("tkn: settings.json 'PreToolUse' is not an array");
        return 1;
    };
    for matcher in ["Bash", "Zsh"] {
        if !arr
            .iter()
            .any(|entry| is_tkn_hook_for_matcher(entry, matcher))
        {
            arr.push(hook_entry(matcher));
        }
    }

    if let Err(e) = write_settings(&settings_path, &settings) {
        eprintln!("tkn: failed to update settings: {e}");
        return 1;
    }

    // Clean up legacy hook script if present
    let legacy_script = claude_dir.join("hooks").join("tkn-hook.sh");
    if legacy_script.exists() {
        let _ = fs::remove_file(&legacy_script);
    }

    println!("tkn hook installed successfully.");
    println!("  Settings: {}", settings_path.display());
    0
}

pub fn uninstall() -> i32 {
    let claude_dir = match dirs::home_dir() {
        Some(h) => h.join(".claude"),
        None => {
            eprintln!("tkn: cannot determine home directory");
            return 1;
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
            return 1;
        }
    }

    println!("tkn hook uninstalled successfully.");
    0
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

fn is_tkn_hook_for_matcher(entry: &serde_json::Value, matcher: &str) -> bool {
    entry
        .get("matcher")
        .and_then(|m| m.as_str())
        .map(|m| m == matcher)
        .unwrap_or(false)
        && is_tkn_hook(entry)
}

fn hook_entry(matcher: &str) -> serde_json::Value {
    serde_json::json!({
        "matcher": matcher,
        "hooks": [{
            "type": "command",
            "command": "tkn hook run"
        }]
    })
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
    let json = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    fs::write(path, json)
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn run_is_noop_on_invalid_input() {
        run();
    }
}
