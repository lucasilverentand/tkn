use std::fs;
use std::path::{Path, PathBuf};

use crate::cmd::hook;
use crate::integration::{ensure_parent_dir, resolve_setup_repo_path, upsert_codex_managed_block};
use crate::storage::StorageManager;
use crate::AssistantTarget;

pub fn run(target: AssistantTarget, repo: Option<&Path>) -> i32 {
    match target {
        AssistantTarget::Claude => match setup_claude() {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("tkn: {e}");
                1
            }
        },
        AssistantTarget::Codex => match setup_codex(repo) {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("tkn: {e}");
                1
            }
        },
        AssistantTarget::All => {
            let claude_result = setup_claude();
            let codex_result = setup_codex(repo);

            match (&claude_result, &codex_result) {
                (Ok(_), Ok(_)) => 0,
                (Ok(_), Err(err)) => {
                    eprintln!("tkn: Claude setup succeeded, Codex setup failed: {err}");
                    1
                }
                (Err(err), Ok(_)) => {
                    eprintln!("tkn: Codex setup succeeded, Claude setup failed: {err}");
                    1
                }
                (Err(claude_err), Err(codex_err)) => {
                    eprintln!("tkn: Claude setup failed: {claude_err}");
                    eprintln!("tkn: Codex setup failed: {codex_err}");
                    1
                }
            }
        }
    }
}

fn setup_claude() -> Result<(), String> {
    let storage = StorageManager::new();
    storage
        .init()
        .map_err(|e| format!("failed to initialize storage: {e}"))?;

    let home_dir = dirs::home_dir().ok_or_else(|| "cannot determine home directory".to_string())?;
    let settings_path = hook::install_for_home(&home_dir)?;
    println!("Claude setup: ready");
    println!("  Hook: {}", settings_path.display());
    println!("  Next: tkn doctor claude");
    Ok(())
}

fn setup_codex(repo: Option<&Path>) -> Result<(), String> {
    let paths = setup_codex_target(repo)?;
    println!("Codex setup: ready");
    println!("  Scope: {}", paths.scope);
    println!("  Config: {}", paths.config_path.display());
    println!("  Hooks: {}", paths.hooks_path.display());
    println!("  AGENTS: {}", paths.agents_path.display());
    println!("  Next: {}", paths.doctor_command);
    Ok(())
}

pub(crate) struct CodexSetupPaths {
    pub scope: String,
    pub agents_path: PathBuf,
    pub config_path: PathBuf,
    pub hooks_path: PathBuf,
    pub doctor_command: String,
}

pub(crate) fn setup_codex_target(repo: Option<&Path>) -> Result<CodexSetupPaths, String> {
    match repo {
        Some(repo) => {
            let repo_path = resolve_setup_repo_path(Some(repo))?;
            setup_codex_repo(&repo_path)
        }
        None => {
            let home_dir =
                dirs::home_dir().ok_or_else(|| "cannot determine home directory".to_string())?;
            setup_codex_home(&home_dir)
        }
    }
}

pub(crate) fn setup_codex_home(home_dir: &Path) -> Result<CodexSetupPaths, String> {
    let codex_dir = hook::codex_home_dir(home_dir);
    setup_codex_paths(
        "global".to_string(),
        codex_dir.join("AGENTS.md"),
        hook::codex_global_config_path(home_dir),
        hook::codex_global_hooks_path(home_dir),
        "tkn doctor codex".to_string(),
    )
}

pub(crate) fn setup_codex_repo(repo_path: &Path) -> Result<CodexSetupPaths, String> {
    setup_codex_paths(
        format!("repo ({})", repo_path.display()),
        repo_path.join("AGENTS.md"),
        hook::codex_config_path(repo_path),
        hook::codex_hooks_path(repo_path),
        format!("tkn doctor codex --repo {}", repo_path.display()),
    )
}

fn setup_codex_paths(
    scope: String,
    agents_path: PathBuf,
    config_path: PathBuf,
    hooks_path: PathBuf,
    doctor_command: String,
) -> Result<CodexSetupPaths, String> {
    let existing = if agents_path.exists() {
        fs::read_to_string(&agents_path)
            .map_err(|e| format!("failed to read {}: {e}", agents_path.display()))?
    } else {
        String::new()
    };
    let updated = upsert_codex_managed_block(&existing);

    ensure_parent_dir(&agents_path)
        .map_err(|e| format!("failed to prepare {}: {e}", agents_path.display()))?;
    fs::write(&agents_path, updated)
        .map_err(|e| format!("failed to write {}: {e}", agents_path.display()))?;

    ensure_parent_dir(&config_path)
        .map_err(|e| format!("failed to prepare {}: {e}", config_path.display()))?;
    ensure_codex_hooks_feature(&config_path)?;

    ensure_parent_dir(&hooks_path)
        .map_err(|e| format!("failed to prepare {}: {e}", hooks_path.display()))?;
    let mut hooks = hook::read_settings(&hooks_path)?;
    hook::repair_codex_hook_settings(&mut hooks)?;
    hook::write_settings(&hooks_path, &hooks)
        .map_err(|e| format!("failed to write {}: {e}", hooks_path.display()))?;

    Ok(CodexSetupPaths {
        scope,
        agents_path,
        config_path,
        hooks_path,
        doctor_command,
    })
}

fn ensure_codex_hooks_feature(config_path: &Path) -> Result<(), String> {
    let existing = if config_path.exists() {
        fs::read_to_string(config_path)
            .map_err(|e| format!("failed to read {}: {e}", config_path.display()))?
    } else {
        String::new()
    };

    if !existing.trim().is_empty() {
        toml::from_str::<toml::Value>(&existing)
            .map_err(|e| format!("invalid TOML in {}: {e}", config_path.display()))?;
    }

    let updated = upsert_codex_hooks_feature(&existing);
    fs::write(config_path, updated)
        .map_err(|e| format!("failed to write {}: {e}", config_path.display()))
}

fn upsert_codex_hooks_feature(existing: &str) -> String {
    let normalized = existing.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.lines().collect();

    let mut in_features = false;
    let mut features_start = None;
    let mut features_end = lines.len();
    let mut hooks_line = None;
    let mut codex_hooks_line = None;

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_features {
                features_end = idx;
                break;
            }
            if trimmed == "[features]" {
                in_features = true;
                features_start = Some(idx);
            }
            continue;
        }

        if in_features {
            let key = trimmed.split('=').next().map(str::trim).unwrap_or("");
            if key == "hooks" {
                hooks_line = Some(idx);
            } else if key == "codex_hooks" {
                codex_hooks_line = Some(idx);
            }
        }
    }

    if features_start.is_none() {
        let trimmed = normalized.trim_end_matches('\n');
        if trimmed.is_empty() {
            return "[features]\nhooks = true\n".to_string();
        }
        return format!("{trimmed}\n\n[features]\nhooks = true\n");
    }

    let mut output = Vec::with_capacity(lines.len() + 1);
    let insert_at = features_end;
    for (idx, line) in lines.iter().enumerate() {
        if Some(idx) == hooks_line {
            output.push("hooks = true".to_string());
        } else if Some(idx) == codex_hooks_line {
            // Drop the deprecated alias while still allowing insertion below.
        } else {
            output.push((*line).to_string());
        }
        if hooks_line.is_none() && idx + 1 == insert_at {
            output.push("hooks = true".to_string());
        }
    }

    let mut joined = output.join("\n");
    joined.push('\n');
    joined
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;

    use crate::integration::{codex_block_matches_current, CODEX_BEGIN_MARKER};

    use super::*;

    #[test]
    fn setup_codex_repo_creates_agents_when_missing() {
        let repo = env::temp_dir().join(format!("tkn-setup-codex-create-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(repo.join(".git")).unwrap();

        let paths = setup_codex_repo(&repo).unwrap();
        let content = fs::read_to_string(paths.agents_path).unwrap();
        assert!(content.contains(CODEX_BEGIN_MARKER));
        assert!(codex_block_matches_current(&content));
        assert!(paths.config_path.exists());
        assert!(paths.hooks_path.exists());

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn setup_codex_home_creates_global_config() {
        let home = env::temp_dir().join(format!("tkn-setup-codex-home-{}", uuid::Uuid::new_v4()));

        let paths = setup_codex_home(&home).unwrap();
        let content = fs::read_to_string(&paths.agents_path).unwrap();
        let hooks = hook::read_settings(&paths.hooks_path).unwrap();

        assert_eq!(paths.scope, "global");
        assert_eq!(paths.config_path, home.join(".codex").join("config.toml"));
        assert_eq!(paths.hooks_path, home.join(".codex").join("hooks.json"));
        assert_eq!(paths.agents_path, home.join(".codex").join("AGENTS.md"));
        assert!(content.contains(CODEX_BEGIN_MARKER));
        assert!(codex_block_matches_current(&content));
        assert!(fs::read_to_string(&paths.config_path)
            .unwrap()
            .contains("hooks = true"));
        assert_eq!(
            hook::hook_entries_for_event_matcher(&hooks, "PreToolUse", hook::TKN_CODEX_MATCHER)
                .len(),
            1
        );
        assert_eq!(
            hook::hook_entries_for_event_matcher(&hooks, "PostToolUse", hook::TKN_CODEX_MATCHER)
                .len(),
            1
        );

        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn setup_codex_repo_preserves_existing_user_content() {
        let repo =
            env::temp_dir().join(format!("tkn-setup-codex-preserve-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(repo.join(".git")).unwrap();
        let agents_path = repo.join("AGENTS.md");
        fs::write(&agents_path, "# Notes\n\nKeep this.\n").unwrap();

        setup_codex_repo(&repo).unwrap();
        let content = fs::read_to_string(&agents_path).unwrap();
        assert!(content.starts_with("# Notes\n\nKeep this.\n\n"));
        assert!(content.contains("## tkn Codex Workflow"));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn setup_codex_repo_updates_stale_managed_block() {
        let repo = env::temp_dir().join(format!("tkn-setup-codex-stale-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(repo.join(".git")).unwrap();
        let agents_path = repo.join("AGENTS.md");
        fs::write(
            &agents_path,
            "<!-- tkn:begin codex -->\nstale\n<!-- tkn:end codex -->\n",
        )
        .unwrap();

        setup_codex_repo(&repo).unwrap();
        let content = fs::read_to_string(&agents_path).unwrap();
        assert!(codex_block_matches_current(&content));
        assert!(!content.contains("\nstale\n"));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn setup_codex_repo_is_idempotent() {
        let repo = env::temp_dir().join(format!(
            "tkn-setup-codex-idempotent-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(repo.join(".git")).unwrap();

        let first = setup_codex_repo(&repo).unwrap();
        let first_content = fs::read_to_string(&first.agents_path).unwrap();
        setup_codex_repo(&repo).unwrap();
        let second_content = fs::read_to_string(&first.agents_path).unwrap();

        assert_eq!(first_content, second_content);

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn setup_codex_repo_preserves_config_and_enables_hooks() {
        let repo = env::temp_dir().join(format!("tkn-setup-codex-config-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(repo.join(".git")).unwrap();
        let config_path = hook::codex_config_path(&repo);
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(
            &config_path,
            "[features]\napps = true\n\n[tools]\nweb_search = true\n",
        )
        .unwrap();

        setup_codex_repo(&repo).unwrap();
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("[features]"));
        assert!(content.contains("apps = true"));
        assert!(content.contains("hooks = true"));
        assert!(!content.contains("codex_hooks"));
        assert!(content.contains("[tools]\nweb_search = true"));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn setup_codex_repo_migrates_deprecated_codex_hooks_feature() {
        let repo =
            env::temp_dir().join(format!("tkn-setup-codex-migrate-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(repo.join(".git")).unwrap();
        let config_path = hook::codex_config_path(&repo);
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(
            &config_path,
            "[features]\napps = true\ncodex_hooks = true\n\n[tools]\nweb_search = true\n",
        )
        .unwrap();

        setup_codex_repo(&repo).unwrap();
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("hooks = true"));
        assert!(!content.contains("codex_hooks"));
        assert!(content.contains("[tools]\nweb_search = true"));

        let _ = fs::remove_dir_all(repo);
    }
}
