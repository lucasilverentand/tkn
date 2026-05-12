use std::fs;
use std::path::Path;

use crate::cmd::hook;
use crate::integration::{self, CheckResult, CheckStatus, DoctorReport, TargetReport};
use crate::storage::StorageManager;
use crate::tool_config::{self, ToolConfig};
use crate::AssistantTarget;

pub fn run(target: Option<AssistantTarget>, repo: Option<&Path>, json: bool) -> i32 {
    let report = build_report(target.unwrap_or(AssistantTarget::All), repo);

    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        render_human(&report);
    }

    if report.has_failures() {
        1
    } else {
        0
    }
}

fn build_report(target: AssistantTarget, repo: Option<&Path>) -> DoctorReport {
    let mut reports = vec![doctor_core()];

    match target {
        AssistantTarget::Claude => reports.push(doctor_claude()),
        AssistantTarget::Codex => reports.push(doctor_codex(repo)),
        AssistantTarget::All => {
            reports.push(doctor_claude());
            reports.push(doctor_codex(repo));
        }
    }

    DoctorReport { reports }
}

fn doctor_core() -> TargetReport {
    let storage = StorageManager::new();
    let mut checks = Vec::new();

    match storage.init() {
        Ok(_) => checks.push(CheckResult::pass(
            "storage initialization",
            format!("initialized {}", storage.base_dir.display()),
        )),
        Err(e) => {
            checks.push(CheckResult::fail(
                "storage initialization",
                format!("failed to initialize {}: {e}", storage.base_dir.display()),
                "Check permissions for ~/.tkn and rerun `tkn doctor`.",
            ));
            return TargetReport {
                target: "core".to_string(),
                checks,
            };
        }
    }

    checks.push(check_dir_writable("logs directory", &storage.logs_dir()));
    checks.push(check_dir_writable(
        "plugins directory",
        &storage.tools_dir(),
    ));
    checks.push(check_builtin_registry());

    TargetReport {
        target: "core".to_string(),
        checks,
    }
}

fn doctor_claude() -> TargetReport {
    let Some(home_dir) = dirs::home_dir() else {
        return TargetReport {
            target: "claude".to_string(),
            checks: vec![CheckResult::fail(
                "home directory",
                "cannot determine home directory",
                "Set HOME correctly and rerun `tkn setup claude`.",
            )],
        };
    };

    doctor_claude_at(&home_dir)
}

fn doctor_claude_at(home_dir: &Path) -> TargetReport {
    let mut checks = Vec::new();
    let settings_path = hook::claude_settings_path(home_dir);
    if !settings_path.exists() {
        checks.push(CheckResult::fail(
            "Claude settings",
            format!("{} does not exist", settings_path.display()),
            "Run `tkn setup claude`.",
        ));
        return TargetReport {
            target: "claude".to_string(),
            checks,
        };
    }

    let settings = match hook::read_settings(&settings_path) {
        Ok(settings) => {
            checks.push(CheckResult::pass(
                "Claude settings JSON",
                format!("parsed {}", settings_path.display()),
            ));
            settings
        }
        Err(e) => {
            checks.push(CheckResult::fail(
                "Claude settings JSON",
                e,
                "Repair the file or rerun `tkn setup claude`.",
            ));
            return TargetReport {
                target: "claude".to_string(),
                checks,
            };
        }
    };

    for matcher in ["Bash", "Zsh"] {
        checks.push(check_claude_matcher(&settings, "PreToolUse", matcher));
        checks.push(check_claude_matcher(&settings, "PostToolUse", matcher));
    }

    TargetReport {
        target: "claude".to_string(),
        checks,
    }
}

fn doctor_codex(repo: Option<&Path>) -> TargetReport {
    let target_result = resolve_codex_doctor_target(repo);
    doctor_codex_for_target(target_result, integration::command_on_path("codex"))
}

#[derive(Clone, Debug)]
struct CodexDoctorTarget {
    discovery_name: &'static str,
    discovery_message: String,
    config_path: std::path::PathBuf,
    hooks_path: std::path::PathBuf,
    agents_path: std::path::PathBuf,
    setup_command: String,
}

fn resolve_codex_doctor_target(repo: Option<&Path>) -> Result<CodexDoctorTarget, String> {
    match repo {
        Some(repo) => {
            let repo_path = integration::resolve_doctor_repo_path(Some(repo))?;
            Ok(CodexDoctorTarget {
                discovery_name: "repository discovery",
                discovery_message: format!("using {}", repo_path.display()),
                config_path: hook::codex_config_path(&repo_path),
                hooks_path: hook::codex_hooks_path(&repo_path),
                agents_path: repo_path.join("AGENTS.md"),
                setup_command: format!("tkn setup codex --repo {}", repo_path.display()),
            })
        }
        None => {
            let home_dir =
                dirs::home_dir().ok_or_else(|| "cannot determine home directory".to_string())?;
            Ok(CodexDoctorTarget {
                discovery_name: "global Codex home",
                discovery_message: format!("using {}", hook::codex_home_dir(&home_dir).display()),
                config_path: hook::codex_global_config_path(&home_dir),
                hooks_path: hook::codex_global_hooks_path(&home_dir),
                agents_path: hook::codex_home_dir(&home_dir).join("AGENTS.md"),
                setup_command: "tkn setup codex".to_string(),
            })
        }
    }
}

fn doctor_codex_for_target(
    target_result: Result<CodexDoctorTarget, String>,
    codex_on_path: bool,
) -> TargetReport {
    let mut checks = Vec::new();

    if codex_on_path {
        checks.push(CheckResult::pass("Codex binary", "`codex` found on PATH"));
    } else {
        checks.push(CheckResult::fail(
            "Codex binary",
            "`codex` not found on PATH",
            "Install Codex and rerun `tkn doctor codex`.",
        ));
    }

    let target = match target_result {
        Ok(target) => {
            checks.push(CheckResult::pass(
                target.discovery_name,
                target.discovery_message.clone(),
            ));
            target
        }
        Err(e) => {
            checks.push(CheckResult::fail(
                "Codex target discovery",
                e,
                "Run `tkn doctor codex` for global setup or pass `--repo <path>` for repo setup.",
            ));
            return TargetReport {
                target: "codex".to_string(),
                checks,
            };
        }
    };

    checks.push(check_codex_config(
        &target.config_path,
        &target.setup_command,
    ));
    checks.push(check_codex_hook(
        &target.hooks_path,
        &target.setup_command,
        "PostToolUse",
    ));

    let agents_path = &target.agents_path;
    if !agents_path.exists() {
        checks.push(CheckResult::fail(
            "AGENTS.md",
            format!("{} does not exist", agents_path.display()),
            format!("Run `{}`.", target.setup_command),
        ));
    } else if let Ok(agents_content) = fs::read_to_string(agents_path) {
        if integration::codex_block_matches_current(&agents_content) {
            checks.push(CheckResult::pass(
                "Codex managed block",
                "managed tkn block is current",
            ));
        } else {
            checks.push(CheckResult::fail(
                "Codex managed block",
                "managed tkn block is missing or stale",
                format!("Run `{}`.", target.setup_command),
            ));
        }

        let contradictory = integration::contradictory_codex_lines(&agents_content);
        if contradictory.is_empty() {
            checks.push(CheckResult::pass(
                "Contradictory instructions",
                "no obvious bare command examples found outside the managed block",
            ));
        } else {
            checks.push(CheckResult::warn(
                "Contradictory instructions",
                format!(
                    "found {} possible bare command examples",
                    contradictory.len()
                ),
                "Review AGENTS.md and route assistant-facing shell examples through tkn.",
            ));
        }
    } else {
        let err = fs::read_to_string(agents_path).unwrap_err();
        checks.push(CheckResult::fail(
            "AGENTS.md",
            format!("failed to read {}: {err}", agents_path.display()),
            format!("Check permissions for {}.", agents_path.display()),
        ));
    }

    TargetReport {
        target: "codex".to_string(),
        checks,
    }
}

fn check_codex_config(config_path: &Path, setup_command: &str) -> CheckResult {
    if !config_path.exists() {
        return CheckResult::fail(
            "Codex hooks feature",
            format!("{} does not exist", config_path.display()),
            format!("Run `{setup_command}`."),
        );
    }

    let content = match fs::read_to_string(config_path) {
        Ok(content) => content,
        Err(e) => {
            return CheckResult::fail(
                "Codex hooks feature",
                format!("failed to read {}: {e}", config_path.display()),
                format!("Check permissions for {}.", config_path.display()),
            );
        }
    };

    let value = match toml::from_str::<toml::Value>(&content) {
        Ok(value) => value,
        Err(e) => {
            return CheckResult::fail(
                "Codex hooks feature",
                format!("invalid TOML in {}: {e}", config_path.display()),
                format!("Run `{setup_command}`."),
            );
        }
    };

    if value
        .get("features")
        .and_then(|features| features.get("codex_hooks"))
        .and_then(|enabled| enabled.as_bool())
        == Some(true)
    {
        CheckResult::pass("Codex hooks feature", "features.codex_hooks is enabled")
    } else {
        CheckResult::fail(
            "Codex hooks feature",
            "features.codex_hooks is not enabled",
            format!("Run `{setup_command}`."),
        )
    }
}

fn check_codex_hook(hooks_path: &Path, setup_command: &str, event: &str) -> CheckResult {
    if !hooks_path.exists() {
        return CheckResult::fail(
            format!("Codex {event} hook"),
            format!("{} does not exist", hooks_path.display()),
            format!("Run `{setup_command}`."),
        );
    }

    let settings = match hook::read_settings(hooks_path) {
        Ok(settings) => settings,
        Err(e) => {
            return CheckResult::fail(
                format!("Codex {event} hook"),
                e,
                format!("Run `{setup_command}`."),
            );
        }
    };

    let entries = hook::hook_entries_for_event_matcher(&settings, event, hook::TKN_CODEX_MATCHER);
    let tkn_entries: Vec<_> = entries
        .into_iter()
        .filter(|entry| hook::is_tkn_hook(entry))
        .collect();

    if tkn_entries.is_empty() {
        return CheckResult::fail(
            format!("Codex {event} hook"),
            "missing tkn hook entry",
            format!("Run `{setup_command}`."),
        );
    }

    if tkn_entries.len() > 1 {
        return CheckResult::fail(
            format!("Codex {event} hook"),
            "duplicate tkn hook entries detected",
            format!("Run `{setup_command}`."),
        );
    }

    if !hook::has_expected_codex_hook_command(tkn_entries[0]) {
        return CheckResult::fail(
            format!("Codex {event} hook"),
            "tkn hook entry does not point to `tkn hook run --codex`",
            format!("Run `{setup_command}`."),
        );
    }

    CheckResult::pass(
        format!("Codex {event} hook"),
        "configured with `tkn hook run --codex`",
    )
}

fn check_claude_matcher(settings: &serde_json::Value, event: &str, matcher: &str) -> CheckResult {
    let entries = hook::hook_entries_for_event_matcher(settings, event, matcher);
    let tkn_entries: Vec<_> = entries
        .into_iter()
        .filter(|entry| hook::is_tkn_hook(entry))
        .collect();

    if tkn_entries.is_empty() {
        return CheckResult::fail(
            format!("{matcher} {event} hook"),
            "missing tkn hook entry",
            "Run `tkn setup claude`.",
        );
    }

    if tkn_entries.len() > 1 {
        return CheckResult::fail(
            format!("{matcher} {event} hook"),
            "duplicate tkn hook entries detected",
            "Run `tkn setup claude` to repair duplicate entries.",
        );
    }

    let entry = tkn_entries[0];
    if hook::contains_legacy_hook(entry) {
        return CheckResult::fail(
            format!("{matcher} {event} hook"),
            "legacy tkn hook entry detected",
            "Run `tkn setup claude` to replace the legacy hook.",
        );
    }

    if !hook::has_expected_hook_command(entry) {
        return CheckResult::fail(
            format!("{matcher} {event} hook"),
            "tkn hook entry does not point to `tkn hook run`",
            "Run `tkn setup claude` to repair the hook command.",
        );
    }

    CheckResult::pass(
        format!("{matcher} {event} hook"),
        "configured with `tkn hook run`",
    )
}

fn check_dir_writable(name: &str, path: &Path) -> CheckResult {
    let probe_path = path.join(format!(".tkn-write-test-{}", uuid::Uuid::new_v4()));
    match fs::write(&probe_path, "ok") {
        Ok(_) => {
            let _ = fs::remove_file(&probe_path);
            CheckResult::pass(name, format!("{} is writable", path.display()))
        }
        Err(e) => CheckResult::fail(
            name,
            format!("{} is not writable: {e}", path.display()),
            format!("Fix permissions for {}.", path.display()),
        ),
    }
}

fn check_builtin_registry() -> CheckResult {
    let builtins = tool_config::builtin_plugins();
    if builtins.is_empty() {
        return CheckResult::fail(
            "built-in plugins",
            "no built-in plugins are registered",
            "Reinstall tkn or rebuild the binary.",
        );
    }

    for (bundle, name, content) in builtins {
        if let Err(e) = toml::from_str::<ToolConfig>(content) {
            return CheckResult::fail(
                "built-in plugins",
                format!("failed to parse {bundle}/{name}: {e}"),
                "Reinstall tkn or rebuild the binary.",
            );
        }
    }

    CheckResult::pass(
        "built-in plugins",
        "built-in plugin registry loads successfully",
    )
}

fn render_human(report: &DoctorReport) {
    for target in &report.reports {
        println!("{}:", target.target);
        for check in &target.checks {
            println!(
                "  [{}] {}: {}",
                label_for_status(check.status),
                check.name,
                check.message
            );
            if let Some(remediation) = &check.remediation {
                println!("      fix: {remediation}");
            }
        }
        println!();
    }
}

fn label_for_status(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Pass => "PASS",
        CheckStatus::Warn => "WARN",
        CheckStatus::Fail => "FAIL",
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;

    use crate::cmd::setup::setup_codex_repo;

    use super::*;

    fn codex_repo_target(repo: &Path) -> CodexDoctorTarget {
        CodexDoctorTarget {
            discovery_name: "repository discovery",
            discovery_message: format!("using {}", repo.display()),
            config_path: hook::codex_config_path(repo),
            hooks_path: hook::codex_hooks_path(repo),
            agents_path: repo.join("AGENTS.md"),
            setup_command: format!("tkn setup codex --repo {}", repo.display()),
        }
    }

    #[test]
    fn doctor_json_output_shape_is_stable() {
        let report = DoctorReport {
            reports: vec![TargetReport {
                target: "core".to_string(),
                checks: vec![CheckResult::pass("storage", "ok")],
            }],
        };

        let value = serde_json::to_value(&report).unwrap();
        assert!(value.get("reports").is_some());
        assert_eq!(value["reports"][0]["target"], "core");
        assert_eq!(value["reports"][0]["checks"][0]["status"], "pass");
    }

    #[test]
    fn doctor_claude_reports_missing_settings() {
        let home = env::temp_dir().join(format!(
            "tkn-doctor-claude-missing-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&home).unwrap();

        let report = doctor_claude_at(&home);
        assert!(report.has_failures());
        assert_eq!(report.checks[0].name, "Claude settings");

        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn doctor_claude_reports_invalid_json() {
        let home = env::temp_dir().join(format!(
            "tkn-doctor-claude-invalid-{}",
            uuid::Uuid::new_v4()
        ));
        let settings_path = hook::claude_settings_path(&home);
        fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        fs::write(&settings_path, "{invalid").unwrap();

        let report = doctor_claude_at(&home);
        assert!(report.has_failures());
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.name == "Claude settings JSON"
                    && check.status == CheckStatus::Fail)
        );

        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn doctor_claude_reports_duplicate_hooks() {
        let home = env::temp_dir().join(format!("tkn-doctor-claude-dup-{}", uuid::Uuid::new_v4()));
        let settings_path = hook::claude_settings_path(&home);
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
                            "hooks": [{"type": "command", "command": "tkn hook run"}]
                        },
                        {
                            "matcher": "Zsh",
                            "hooks": [{"type": "command", "command": "tkn hook run"}]
                        }
                    ]
                }
            })
            .to_string(),
        )
        .unwrap();

        let report = doctor_claude_at(&home);
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.name == "Bash PreToolUse hook"
                    && check.status == CheckStatus::Fail)
        );

        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn doctor_claude_passes_after_setup_repair() {
        let home = env::temp_dir().join(format!("tkn-doctor-claude-ok-{}", uuid::Uuid::new_v4()));
        hook::install_for_home(&home).unwrap();

        let report = doctor_claude_at(&home);
        assert!(!report.has_failures());

        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn doctor_codex_reports_missing_agents() {
        let repo =
            env::temp_dir().join(format!("tkn-doctor-codex-missing-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(repo.join(".git")).unwrap();

        let report = doctor_codex_for_target(Ok(codex_repo_target(&repo)), true);
        assert!(report
            .checks
            .iter()
            .any(|check| check.name == "AGENTS.md" && check.status == CheckStatus::Fail));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn doctor_codex_warns_on_contradictory_instructions() {
        let repo = env::temp_dir().join(format!("tkn-doctor-codex-warn-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(repo.join(".git")).unwrap();
        setup_codex_repo(&repo).unwrap();
        let agents_path = repo.join("AGENTS.md");
        let existing = fs::read_to_string(&agents_path).unwrap();
        fs::write(
            &agents_path,
            format!("# Notes\n\n- Run `cargo test`\n\n{existing}"),
        )
        .unwrap();

        let report = doctor_codex_for_target(Ok(codex_repo_target(&repo)), true);
        assert!(report
            .checks
            .iter()
            .any(|check| check.name == "Contradictory instructions"
                && check.status == CheckStatus::Warn));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn doctor_codex_passes_with_current_managed_block() {
        let repo = env::temp_dir().join(format!("tkn-doctor-codex-ok-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(repo.join(".git")).unwrap();
        setup_codex_repo(&repo).unwrap();

        let report = doctor_codex_for_target(Ok(codex_repo_target(&repo)), true);
        assert!(report
            .checks
            .iter()
            .any(|check| check.name == "Codex managed block" && check.status == CheckStatus::Pass));

        let _ = fs::remove_dir_all(repo);
    }
}
