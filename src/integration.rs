use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub const CODEX_BEGIN_MARKER: &str = "<!-- tkn:begin codex -->";
pub const CODEX_END_MARKER: &str = "<!-- tkn:end codex -->";
pub const CODEX_TEMPLATE_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct TargetReport {
    pub target: String,
    pub checks: Vec<CheckResult>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct DoctorReport {
    pub reports: Vec<TargetReport>,
}

impl CheckResult {
    pub fn pass(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Pass,
            message: message.into(),
            remediation: None,
        }
    }

    pub fn warn(
        name: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Warn,
            message: message.into(),
            remediation: Some(remediation.into()),
        }
    }

    pub fn fail(
        name: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Fail,
            message: message.into(),
            remediation: Some(remediation.into()),
        }
    }
}

impl TargetReport {
    pub fn has_failures(&self) -> bool {
        self.checks
            .iter()
            .any(|check| check.status == CheckStatus::Fail)
    }
}

impl DoctorReport {
    pub fn has_failures(&self) -> bool {
        self.reports.iter().any(TargetReport::has_failures)
    }
}

pub fn command_on_path(command: &str) -> bool {
    let Some(path_var) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&path_var).any(|dir| {
        let candidate = dir.join(command);
        if candidate.is_file() {
            return true;
        }

        #[cfg(windows)]
        {
            let candidate = dir.join(format!("{command}.exe"));
            candidate.is_file()
        }

        #[cfg(not(windows))]
        {
            false
        }
    })
}

pub fn is_git_repo(path: &Path) -> bool {
    let git_path = path.join(".git");
    git_path.is_dir() || git_path.is_file()
}

pub fn infer_repo_from_cwd() -> Result<PathBuf, String> {
    let cwd = env::current_dir().map_err(|e| format!("failed to read current directory: {e}"))?;
    find_git_repo(&cwd).ok_or_else(|| {
        "current directory is not inside a git repository; pass --repo <path>".to_string()
    })
}

pub fn resolve_setup_repo_path(repo: Option<&Path>) -> Result<PathBuf, String> {
    match repo {
        Some(path) => canonicalize_repo(path),
        None => {
            let cwd =
                env::current_dir().map_err(|e| format!("failed to read current directory: {e}"))?;
            if is_git_repo(&cwd) {
                canonicalize_repo(&cwd)
            } else {
                Err("current directory is not a git repository; pass --repo <path>".to_string())
            }
        }
    }
}

pub fn resolve_doctor_repo_path(repo: Option<&Path>) -> Result<PathBuf, String> {
    match repo {
        Some(path) => canonicalize_repo(path),
        None => infer_repo_from_cwd().and_then(|path| canonicalize_repo(&path)),
    }
}

fn canonicalize_repo(path: &Path) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|e| format!("failed to resolve repo path {}: {e}", path.display()))?;
    if is_git_repo(&canonical) {
        Ok(canonical)
    } else {
        Err(format!("{} is not a git repository", canonical.display()))
    }
}

pub fn find_git_repo(start: &Path) -> Option<PathBuf> {
    let mut current = start;
    loop {
        if is_git_repo(current) {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

pub fn codex_managed_block() -> String {
    format!(
        "{CODEX_BEGIN_MARKER}\n\
<!-- tkn:template-version {CODEX_TEMPLATE_VERSION} -->\n\
## tkn Codex Workflow\n\
\n\
- Codex `PreToolUse` hooks cannot currently rewrite command input, so the repo instructions are the source of truth for routing commands through `tkn`.\n\
- Default to `tkn auto -- <command>`.\n\
- Use `tkn exec -- <command>` for deterministic captured output.\n\
- Use `tkn pass -- <command>` for interactive, long-lived, or streaming commands.\n\
- Do not wrap `tkn` with `tkn`.\n\
{CODEX_END_MARKER}\n"
    )
}

pub fn upsert_codex_managed_block(existing: &str) -> String {
    let block = codex_managed_block();
    let normalized = normalize_newlines(existing);

    if let Some((start, end)) = codex_block_range(&normalized) {
        let mut updated = String::with_capacity(normalized.len() + block.len());
        updated.push_str(normalized[..start].trim_end_matches('\n'));
        if !updated.is_empty() {
            updated.push_str("\n\n");
        }
        updated.push_str(&block);
        let suffix = normalized[end..].trim_start_matches('\n');
        if !suffix.is_empty() {
            updated.push('\n');
            updated.push('\n');
            updated.push_str(suffix);
            if !updated.ends_with('\n') {
                updated.push('\n');
            }
        }
        return updated;
    }

    let trimmed = normalized.trim_end_matches('\n');
    if trimmed.is_empty() {
        block
    } else {
        format!("{trimmed}\n\n{block}")
    }
}

pub fn codex_block_matches_current(content: &str) -> bool {
    match extract_codex_managed_block(content) {
        Some(block) => block == codex_managed_block().trim_end(),
        None => false,
    }
}

pub fn extract_codex_managed_block(content: &str) -> Option<String> {
    let normalized = normalize_newlines(content);
    let (start, end) = codex_block_range(&normalized)?;
    Some(normalized[start..end].trim_end().to_string())
}

pub fn content_without_codex_block(content: &str) -> String {
    let normalized = normalize_newlines(content);
    if let Some((start, end)) = codex_block_range(&normalized) {
        let mut stripped = String::new();
        stripped.push_str(normalized[..start].trim_end_matches('\n'));
        if !stripped.is_empty() && !normalized[end..].trim().is_empty() {
            stripped.push('\n');
            stripped.push('\n');
        }
        stripped.push_str(normalized[end..].trim_start_matches('\n'));
        return stripped;
    }
    normalized
}

pub fn contradictory_codex_lines(content: &str) -> Vec<String> {
    let stripped = content_without_codex_block(content);
    let mut lines = Vec::new();

    for raw_line in stripped.lines() {
        let line = raw_line.trim();
        if line.is_empty()
            || line.contains("tkn auto --")
            || line.contains("tkn exec --")
            || line.contains("tkn pass --")
        {
            continue;
        }

        if line.contains("`cargo ")
            || line.contains("`git ")
            || line.contains("`rg ")
            || line.contains("`grep ")
            || line.contains("`find ")
            || line.contains("`npm ")
            || line.contains("`pnpm ")
            || line.contains("`yarn ")
            || line.contains("`bun ")
            || line.contains("`docker ")
            || line.contains("`kubectl ")
        {
            lines.push(line.to_string());
        }
    }

    lines
}

pub fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn normalize_newlines(content: &str) -> String {
    content.replace("\r\n", "\n")
}

fn codex_block_range(content: &str) -> Option<(usize, usize)> {
    let start = content.find(CODEX_BEGIN_MARKER)?;
    let end_marker = content[start..].find(CODEX_END_MARKER)? + start;
    let end = content[end_marker..]
        .find('\n')
        .map(|offset| end_marker + offset + 1)
        .unwrap_or(content.len());
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_codex_block_into_empty_agents() {
        let updated = upsert_codex_managed_block("");
        assert_eq!(updated, codex_managed_block());
    }

    #[test]
    fn updates_existing_codex_block_in_place() {
        let original = format!("Intro\n\n{CODEX_BEGIN_MARKER}\nold\n{CODEX_END_MARKER}\n\nOutro\n");
        let updated = upsert_codex_managed_block(&original);
        assert!(updated.contains("## tkn Codex Workflow"));
        assert!(updated.starts_with("Intro\n\n"));
        assert!(updated.ends_with("Outro\n"));
    }

    #[test]
    fn preserves_non_managed_content_around_codex_block() {
        let original = "Top\n\nMiddle\n";
        let updated = upsert_codex_managed_block(original);
        assert!(updated.starts_with("Top\n\nMiddle\n\n"));
        assert!(updated.contains(CODEX_BEGIN_MARKER));
    }

    #[test]
    fn detects_current_codex_block() {
        let content = codex_managed_block();
        assert!(codex_block_matches_current(&content));
    }

    #[test]
    fn finds_git_repo_in_parent_directories() {
        let root = env::temp_dir().join(format!("tkn-repo-{}", uuid::Uuid::new_v4()));
        let nested = root.join("nested/deeper");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(&nested).unwrap();

        let found = find_git_repo(&nested).unwrap();
        assert_eq!(found, root);

        let _ = fs::remove_dir_all(found);
    }
}
