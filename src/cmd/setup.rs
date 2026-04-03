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
    let repo_path = resolve_setup_repo_path(repo)?;
    let agents_path = setup_codex_repo(&repo_path)?;
    println!("Codex setup: ready");
    println!("  AGENTS: {}", agents_path.display());
    println!("  Next: tkn doctor codex --repo {}", repo_path.display());
    Ok(())
}

pub(crate) fn setup_codex_repo(repo_path: &Path) -> Result<PathBuf, String> {
    let agents_path = repo_path.join("AGENTS.md");
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

    Ok(agents_path)
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

        let agents_path = setup_codex_repo(&repo).unwrap();
        let content = fs::read_to_string(agents_path).unwrap();
        assert!(content.contains(CODEX_BEGIN_MARKER));
        assert!(codex_block_matches_current(&content));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn setup_codex_repo_preserves_existing_user_content() {
        let repo = env::temp_dir().join(format!("tkn-setup-codex-preserve-{}", uuid::Uuid::new_v4()));
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
        let repo = env::temp_dir().join(format!("tkn-setup-codex-idempotent-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(repo.join(".git")).unwrap();

        let first = setup_codex_repo(&repo).unwrap();
        let first_content = fs::read_to_string(&first).unwrap();
        setup_codex_repo(&repo).unwrap();
        let second_content = fs::read_to_string(&first).unwrap();

        assert_eq!(first_content, second_content);

        let _ = fs::remove_dir_all(repo);
    }
}
