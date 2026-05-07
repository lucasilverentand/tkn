use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::storage::StorageManager;

use super::routing;

pub const TKN_HOOK_COMMAND: &str = "tkn hook run";
pub const TKN_CODEX_HOOK_COMMAND: &str = "tkn hook run --codex";
pub const TKN_CODEX_MATCHER: &str = "^Bash$";
const TKN_MATCHERS: [&str; 2] = ["Bash", "Zsh"];

/// Runs as the hook itself: reads stdin JSON and writes a hook response to stdout.
pub fn run(codex: bool) {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return;
    }

    if let Some(response) = hook_response(&input, codex) {
        print!("{}", serde_json::to_string(&response).unwrap());
    }
}

pub fn install() -> i32 {
    let storage = StorageManager::new();
    if let Err(e) = storage.init() {
        eprintln!("tkn: failed to initialize storage: {e}");
        return 1;
    }

    let home_dir = match dirs::home_dir() {
        Some(home_dir) => home_dir,
        None => {
            eprintln!("tkn: cannot determine home directory");
            return 1;
        }
    };

    match install_for_home(&home_dir) {
        Ok(settings_path) => {
            println!("tkn hook installed successfully.");
            println!("  Settings: {}", settings_path.display());
            0
        }
        Err(e) => {
            eprintln!("tkn: {e}");
            1
        }
    }
}

pub fn install_for_home(home_dir: &Path) -> Result<PathBuf, String> {
    let claude_dir = claude_dir(home_dir);
    fs::create_dir_all(&claude_dir)
        .map_err(|e| format!("failed to create {}: {e}", claude_dir.display()))?;

    let settings_path = claude_settings_path(home_dir);
    let mut settings = read_settings(&settings_path)?;
    repair_hook_settings(&mut settings)?;
    write_settings(&settings_path, &settings)
        .map_err(|e| format!("failed to update settings: {e}"))?;

    let legacy_script = claude_dir.join("hooks").join("tkn-hook.sh");
    if legacy_script.exists() {
        let _ = fs::remove_file(&legacy_script);
    }

    Ok(settings_path)
}

pub fn uninstall() -> i32 {
    let home_dir = match dirs::home_dir() {
        Some(home_dir) => home_dir,
        None => {
            eprintln!("tkn: cannot determine home directory");
            return 1;
        }
    };

    let legacy_script = claude_dir(&home_dir).join("hooks").join("tkn-hook.sh");
    if legacy_script.exists() {
        let _ = fs::remove_file(&legacy_script);
    }

    let settings_path = claude_settings_path(&home_dir);
    if settings_path.exists() {
        let mut settings = match read_settings(&settings_path) {
            Ok(settings) => settings,
            Err(e) => {
                eprintln!("tkn: {e}");
                return 1;
            }
        };

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

pub fn claude_dir(home_dir: &Path) -> PathBuf {
    home_dir.join(".claude")
}

pub fn claude_settings_path(home_dir: &Path) -> PathBuf {
    claude_dir(home_dir).join("settings.json")
}

pub fn codex_dir(repo_dir: &Path) -> PathBuf {
    repo_dir.join(".codex")
}

pub fn codex_config_path(repo_dir: &Path) -> PathBuf {
    codex_dir(repo_dir).join("config.toml")
}

pub fn codex_hooks_path(repo_dir: &Path) -> PathBuf {
    codex_dir(repo_dir).join("hooks.json")
}

pub fn read_settings(path: &Path) -> Result<serde_json::Value, String> {
    if path.exists() {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("invalid JSON in {}: {e}", path.display()))
    } else {
        Ok(serde_json::json!({}))
    }
}

pub fn repair_hook_settings(settings: &mut serde_json::Value) -> Result<(), String> {
    let Some(settings_obj) = settings.as_object_mut() else {
        return Err("settings.json is not a JSON object".to_string());
    };

    let hooks = settings_obj
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let Some(hooks_obj) = hooks.as_object_mut() else {
        return Err("settings.json 'hooks' is not an object".to_string());
    };

    let pre_tool_use = hooks_obj
        .entry("PreToolUse")
        .or_insert_with(|| serde_json::json!([]));

    let Some(arr) = pre_tool_use.as_array_mut() else {
        return Err("settings.json 'PreToolUse' is not an array".to_string());
    };

    arr.retain(|entry| !is_tkn_hook(entry));
    for matcher in TKN_MATCHERS {
        arr.push(hook_entry(matcher));
    }

    Ok(())
}

pub fn repair_codex_hook_settings(settings: &mut serde_json::Value) -> Result<(), String> {
    let Some(settings_obj) = settings.as_object_mut() else {
        return Err("hooks.json is not a JSON object".to_string());
    };

    let hooks = settings_obj
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let Some(hooks_obj) = hooks.as_object_mut() else {
        return Err("hooks.json 'hooks' is not an object".to_string());
    };

    let pre_tool_use = hooks_obj
        .entry("PreToolUse")
        .or_insert_with(|| serde_json::json!([]));

    let Some(arr) = pre_tool_use.as_array_mut() else {
        return Err("hooks.json 'PreToolUse' is not an array".to_string());
    };

    arr.retain(|entry| !is_tkn_hook(entry));
    arr.push(codex_hook_entry());

    Ok(())
}

pub fn hook_entries_for_matcher<'a>(
    settings: &'a serde_json::Value,
    matcher: &str,
) -> Vec<&'a serde_json::Value> {
    settings
        .get("hooks")
        .and_then(|hooks| hooks.get("PreToolUse"))
        .and_then(|pre_tool_use| pre_tool_use.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| {
                    entry.get("matcher").and_then(|value| value.as_str()) == Some(matcher)
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn codex_hook_entries_for_matcher<'a>(
    settings: &'a serde_json::Value,
    matcher: &str,
) -> Vec<&'a serde_json::Value> {
    hook_entries_for_matcher(settings, matcher)
}

pub fn has_expected_hook_command(entry: &serde_json::Value) -> bool {
    entry
        .get("hooks")
        .and_then(|hooks| hooks.as_array())
        .map(|hooks| {
            hooks.iter().any(|hook| {
                hook.get("command")
                    .and_then(|command| command.as_str())
                    .map(|command| command == TKN_HOOK_COMMAND)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

pub fn has_expected_codex_hook_command(entry: &serde_json::Value) -> bool {
    entry
        .get("hooks")
        .and_then(|hooks| hooks.as_array())
        .map(|hooks| {
            hooks.iter().any(|hook| {
                hook.get("command")
                    .and_then(|command| command.as_str())
                    .map(|command| command == TKN_CODEX_HOOK_COMMAND)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

pub fn contains_legacy_hook(entry: &serde_json::Value) -> bool {
    entry
        .get("hooks")
        .and_then(|hooks| hooks.as_array())
        .map(|hooks| {
            hooks.iter().any(|hook| {
                hook.get("command")
                    .and_then(|command| command.as_str())
                    .map(|command| command.contains("tkn") && command != TKN_HOOK_COMMAND)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

pub fn is_tkn_hook(entry: &serde_json::Value) -> bool {
    entry
        .get("hooks")
        .and_then(|hooks| hooks.as_array())
        .map(|hooks| {
            hooks.iter().any(|hook| {
                hook.get("command")
                    .and_then(|command| command.as_str())
                    .map(|command| command.contains("tkn"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn hook_entry(matcher: &str) -> serde_json::Value {
    serde_json::json!({
        "matcher": matcher,
        "hooks": [{
            "type": "command",
            "command": TKN_HOOK_COMMAND
        }]
    })
}

fn codex_hook_entry() -> serde_json::Value {
    serde_json::json!({
        "matcher": TKN_CODEX_MATCHER,
        "hooks": [{
            "type": "command",
            "command": TKN_CODEX_HOOK_COMMAND,
            "timeout": 30,
            "statusMessage": "Checking tkn routing"
        }]
    })
}

pub fn write_settings(path: &Path, value: &serde_json::Value) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    fs::write(path, json)
}

fn hook_response(input: &str, codex: bool) -> Option<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(input).ok()?;
    let command = value
        .pointer("/tool_input/command")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if routing::should_skip(command) {
        return None;
    }

    if codex {
        Some(codex_pretooluse_response(command))
    } else {
        Some(claude_pretooluse_response(command))
    }
}

fn claude_pretooluse_response(command: &str) -> serde_json::Value {
    // Pass the original command as a single shell-quoted argument so it
    // survives arg-splitting (e.g. multi-word git commit messages keep their
    // quoting). Single-quote the value, escaping any embedded single quotes.
    let escaped = command.replace('\'', "'\\''");
    let new_command = format!("tkn auto -- '{escaped}'");
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "updatedInput": {
                "command": new_command
            }
        }
    })
}

fn codex_pretooluse_response(command: &str) -> serde_json::Value {
    let suggested = if routing::is_long_lived(command) {
        format!("tkn pass -- {command}")
    } else {
        format!("tkn auto -- {command}")
    };

    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": format!(
                "Route assistant shell commands through tkn. Rerun this command as `{suggested}`."
            )
        }
    })
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[test]
    fn run_is_noop_on_invalid_input() {
        assert!(hook_response("{invalid", false).is_none());
    }

    #[test]
    fn claude_response_rewrites_simple_command() {
        let input = serde_json::json!({
            "tool_input": {
                "command": "git commit -m 'hello world'"
            }
        })
        .to_string();

        let response = hook_response(&input, false).unwrap();
        assert_eq!(
            response.pointer("/hookSpecificOutput/updatedInput/command"),
            Some(&serde_json::json!(
                "tkn auto -- 'git commit -m '\\''hello world'\\'''"
            ))
        );
    }

    #[test]
    fn codex_response_blocks_simple_command_with_tkn_instruction() {
        let input = serde_json::json!({
            "tool_input": {
                "command": "cargo test"
            }
        })
        .to_string();

        let response = hook_response(&input, true).unwrap();
        assert_eq!(
            response.pointer("/hookSpecificOutput/permissionDecision"),
            Some(&serde_json::json!("deny"))
        );
        assert!(response
            .pointer("/hookSpecificOutput/permissionDecisionReason")
            .and_then(|value| value.as_str())
            .unwrap()
            .contains("tkn auto -- cargo test"));
    }

    #[test]
    fn codex_response_uses_pass_for_long_lived_commands() {
        let input = serde_json::json!({
            "tool_input": {
                "command": "npm run dev"
            }
        })
        .to_string();

        let response = hook_response(&input, true).unwrap();
        assert!(response
            .pointer("/hookSpecificOutput/permissionDecisionReason")
            .and_then(|value| value.as_str())
            .unwrap()
            .contains("tkn pass -- npm run dev"));
    }

    #[test]
    fn codex_response_is_noop_for_already_wrapped_command() {
        let input = serde_json::json!({
            "tool_input": {
                "command": "tkn auto -- cargo test"
            }
        })
        .to_string();

        assert!(hook_response(&input, true).is_none());
    }

    #[test]
    fn repair_codex_hook_settings_repairs_duplicate_entries() {
        let mut settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "^Bash$",
                        "hooks": [{"type": "command", "command": "tkn hook run --codex"}]
                    },
                    {
                        "matcher": "^Bash$",
                        "hooks": [{"type": "command", "command": "tkn hook run"}]
                    }
                ]
            }
        });

        repair_codex_hook_settings(&mut settings).unwrap();
        let entries = codex_hook_entries_for_matcher(&settings, TKN_CODEX_MATCHER);
        assert_eq!(entries.len(), 1);
        assert!(has_expected_codex_hook_command(entries[0]));
    }

    #[test]
    fn install_for_home_repairs_duplicate_and_legacy_entries() {
        let home = env::temp_dir().join(format!("tkn-hook-home-{}", uuid::Uuid::new_v4()));
        let settings_path = claude_settings_path(&home);
        fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        fs::write(
            &settings_path,
            serde_json::json!({
                "hooks": {
                    "PreToolUse": [
                        {
                            "matcher": "Bash",
                            "hooks": [{"type": "command", "command": "tkn hook run"}]
                        },
                        {
                            "matcher": "Bash",
                            "hooks": [{"type": "command", "command": "~/.claude/hooks/tkn-hook.sh"}]
                        }
                    ]
                }
            })
            .to_string(),
        )
        .unwrap();

        install_for_home(&home).unwrap();
        let settings = read_settings(&settings_path).unwrap();
        assert_eq!(hook_entries_for_matcher(&settings, "Bash").len(), 1);
        assert_eq!(hook_entries_for_matcher(&settings, "Zsh").len(), 1);
        assert!(has_expected_hook_command(
            hook_entries_for_matcher(&settings, "Bash")[0]
        ));

        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn read_settings_rejects_invalid_json() {
        let home = env::temp_dir().join(format!("tkn-hook-invalid-{}", uuid::Uuid::new_v4()));
        let settings_path = claude_settings_path(&home);
        fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        fs::write(&settings_path, "{invalid").unwrap();

        let err = read_settings(&settings_path).unwrap_err();
        assert!(err.contains("invalid JSON"));

        let _ = fs::remove_dir_all(home);
    }
}
