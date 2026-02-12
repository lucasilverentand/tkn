use std::fs;
use std::path::PathBuf;

pub fn install() {
    let claude_dir = match dirs::home_dir() {
        Some(h) => h.join(".claude"),
        None => {
            eprintln!("tkn: cannot determine home directory");
            return;
        }
    };

    // Ensure hooks directory exists
    let hooks_dir = claude_dir.join("hooks");
    if let Err(e) = fs::create_dir_all(&hooks_dir) {
        eprintln!("tkn: failed to create hooks dir: {e}");
        return;
    }

    // Write the hook script
    let hook_script_path = hooks_dir.join("tkn-hook.sh");
    let hook_script = r#"#!/usr/bin/env bash
# tkn hook - rewrites Bash commands to route through tkn exec
# Installed by: tkn hook install

set -euo pipefail

INPUT=$(cat)

COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')
if [ -z "$COMMAND" ]; then
    echo "$INPUT"
    exit 0
fi

# Skip if command already starts with tkn (prevent recursion)
if echo "$COMMAND" | grep -q '^tkn '; then
    echo "$INPUT"
    exit 0
fi

# Rewrite command to route through tkn exec
ESCAPED=$(echo "$COMMAND" | jq -Rs '.')
NEW_COMMAND="tkn exec -- $(echo "$ESCAPED" | jq -r '.')"

echo "$INPUT" | jq --arg cmd "$NEW_COMMAND" '.tool_input.command = $cmd'
"#;

    if let Err(e) = fs::write(&hook_script_path, hook_script) {
        eprintln!("tkn: failed to write hook script: {e}");
        return;
    }

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&hook_script_path, fs::Permissions::from_mode(0o755));
    }

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
            "command": hook_script_path.to_string_lossy()
        }]
    });

    // Check if our hook is already installed
    let arr = pre_tool_use.as_array_mut().unwrap();
    let already_installed = arr.iter().any(|entry| {
        entry
            .get("hooks")
            .and_then(|h| h.as_array())
            .map(|hooks| {
                hooks.iter().any(|h| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .map(|c| c.contains("tkn-hook"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    });

    if !already_installed {
        arr.push(hook_entry);
    }

    if let Err(e) = write_settings(&settings_path, &settings) {
        eprintln!("tkn: failed to update settings: {e}");
        return;
    }

    println!("tkn hook installed successfully.");
    println!("  Hook script: {}", hook_script_path.display());
    println!("  Settings:    {}", settings_path.display());
}

pub fn uninstall() {
    let claude_dir = match dirs::home_dir() {
        Some(h) => h.join(".claude"),
        None => {
            eprintln!("tkn: cannot determine home directory");
            return;
        }
    };

    // Remove hook script
    let hook_script_path = claude_dir.join("hooks").join("tkn-hook.sh");
    if hook_script_path.exists() {
        if let Err(e) = fs::remove_file(&hook_script_path) {
            eprintln!("tkn: failed to remove hook script: {e}");
        }
    }

    // Remove from settings.json
    let settings_path = claude_dir.join("settings.json");
    if settings_path.exists() {
        let mut settings = read_settings(&settings_path);

        if let Some(hooks) = settings.get_mut("hooks") {
            if let Some(pre_tool_use) = hooks.get_mut("PreToolUse") {
                if let Some(arr) = pre_tool_use.as_array_mut() {
                    arr.retain(|entry| {
                        !entry
                            .get("hooks")
                            .and_then(|h| h.as_array())
                            .map(|hooks| {
                                hooks.iter().any(|h| {
                                    h.get("command")
                                        .and_then(|c| c.as_str())
                                        .map(|c| c.contains("tkn-hook"))
                                        .unwrap_or(false)
                                })
                            })
                            .unwrap_or(false)
                    });
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
