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

    // Rewrite command to route through tkn exec
    let new_command = format!("tkn exec -- {command}");
    let response = serde_json::json!({
        "hookSpecificOutput": {
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
