mod cmd;
mod integration;
mod optimizer;
mod runner;
mod shell;
mod storage;
mod tool_config;
mod transformer;
mod types;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "tkn",
    about = "Shell proxy for token-optimized AI tool output",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Route a command through optimized or passthrough execution
    Auto {
        /// Command and arguments to run
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Execute a command through the optimization proxy
    Exec {
        /// Command and arguments to run
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Pass a command through with inherited stdio (no capture/optimization)
    Pass {
        /// Command and arguments to run
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Show command counts and output-size savings
    Stats {
        /// Remove a specific tool from stats (e.g. "git diff")
        #[arg(long)]
        reset: Option<String>,
        /// Reset only the failure count for a tool (e.g. "git diff")
        #[arg(long)]
        reset_failures: Option<String>,
    },
    /// Browse stored command logs
    Log {
        /// Reference ID to show full log for
        id: Option<String>,
        /// Reason for reading the full log (required when viewing a log)
        reason: Option<String>,
        /// Line range (e.g., "10:20") or single line number (e.g., "42")
        #[arg(long)]
        lines: Option<String>,
    },
    /// Manage Claude and Codex hooks
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },
    /// Install or repair Claude and Codex integrations
    Setup {
        /// Assistant integration to set up
        #[arg(value_enum)]
        target: AssistantTarget,
        /// Git repository for Codex config, hooks, and AGENTS.md
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Verify tkn storage, plugins, and assistant integrations
    Doctor {
        /// Assistant integration to verify
        #[arg(value_enum)]
        target: Option<AssistantTarget>,
        /// Git repository for Codex config, hooks, and AGENTS.md
        #[arg(long)]
        repo: Option<PathBuf>,
        /// Emit machine-readable JSON output
        #[arg(long)]
        json: bool,
    },
    /// Clear stored stats and/or logs
    Clean {
        /// Only clear logs
        #[arg(long)]
        logs: bool,
        /// Only clear stats
        #[arg(long)]
        stats: bool,
    },
    /// Inspect command history for plugin optimization opportunities
    Analyze {
        #[command(subcommand)]
        action: AnalyzeAction,
    },
    /// Show trends in full log read reasons
    Reasons,
    /// Replay a stored command through the current optimizer pipeline
    Replay {
        /// Reference ID of the log entry to replay
        id: String,
    },
    /// Manage tool plugins
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
}

#[derive(Subcommand)]
enum PluginAction {
    /// Install built-in plugins or plugins from a repository URL
    Install {
        /// Repository URL, or "default"; omit to install built-in defaults
        url: Option<String>,
    },
    /// List built-in and installed plugins
    List,
    /// Remove an installed plugin
    Remove {
        /// Plugin name to remove
        name: String,
    },
}

#[derive(Subcommand)]
enum AnalyzeAction {
    /// Scan all tools and rank by optimization opportunity
    Scan,
    /// Analyze a specific tool's output patterns
    Report {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
enum HookAction {
    /// Install or repair Claude and/or Codex hooks
    Install {
        /// Assistant target (default: all)
        #[arg(value_enum)]
        target: Option<AssistantTarget>,
        /// Git repository for Codex config, hooks, and AGENTS.md
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Remove Claude and/or Codex hooks
    Uninstall {
        /// Assistant target (default: all)
        #[arg(value_enum)]
        target: Option<AssistantTarget>,
        /// Git repository for Codex config, hooks, and AGENTS.md
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Run an assistant hook (reads stdin, writes stdout)
    Run {
        /// Run in Codex PreToolUse mode
        #[arg(long, hide = true)]
        codex: bool,
        /// Extra arguments (ignored, passed by Claude Code)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
        _args: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum AssistantTarget {
    Claude,
    Codex,
    All,
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Auto { args } => cmd::auto::run(&args),
        Commands::Exec { args } => cmd::exec::run(&args),
        Commands::Pass { args } => cmd::pass::run(&args),
        Commands::Stats {
            reset,
            reset_failures,
        } => cmd::stats::run(reset.as_deref(), reset_failures.as_deref()),
        Commands::Log { id, reason, lines } => {
            cmd::logs::run(id.as_deref(), reason.as_deref(), lines.as_deref())
        }
        Commands::Hook { action } => match action {
            HookAction::Install { target, repo } => {
                let target = hook_target_or_default(target, repo.as_ref());
                cmd::hook::install(target, repo.as_deref())
            }
            HookAction::Uninstall { target, repo } => {
                let target = hook_target_or_default(target, repo.as_ref());
                cmd::hook::uninstall(target, repo.as_deref())
            }
            HookAction::Run { codex, .. } => {
                cmd::hook::run(codex);
                0
            }
        },
        Commands::Setup { target, repo } => cmd::setup::run(target, repo.as_deref()),
        Commands::Doctor { target, repo, json } => cmd::doctor::run(target, repo.as_deref(), json),
        Commands::Clean { logs, stats } => cmd::clean::run(logs, stats),
        Commands::Analyze { action } => match action {
            AnalyzeAction::Scan => cmd::analyze::scan(),
            AnalyzeAction::Report { args } => cmd::analyze::report(&args),
        },
        Commands::Reasons => {
            cmd::reasons::run();
            0
        }
        Commands::Replay { id } => cmd::replay::run(&id),
        Commands::Plugin { action } => match action {
            PluginAction::Install { url } => cmd::plugin::install(url.as_deref()),
            PluginAction::List => {
                cmd::plugin::list();
                0
            }
            PluginAction::Remove { name } => cmd::plugin::remove(&name),
        },
    };

    std::process::exit(exit_code);
}

fn hook_target_or_default(
    target: Option<AssistantTarget>,
    _repo: Option<&PathBuf>,
) -> AssistantTarget {
    target.unwrap_or(AssistantTarget::All)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn hook_target_defaults_to_all_without_repo() {
        assert_eq!(hook_target_or_default(None, None), AssistantTarget::All);
    }

    #[test]
    fn hook_target_defaults_to_all_with_repo() {
        let repo = PathBuf::from("/tmp/example");
        assert_eq!(
            hook_target_or_default(None, Some(&repo)),
            AssistantTarget::All
        );
    }

    #[test]
    fn hook_target_preserves_explicit_target() {
        assert_eq!(
            hook_target_or_default(Some(AssistantTarget::Claude), None),
            AssistantTarget::Claude
        );
        assert_eq!(
            hook_target_or_default(Some(AssistantTarget::Codex), None),
            AssistantTarget::Codex
        );
    }

    #[test]
    fn help_output_describes_current_hook_defaults() {
        let mut command = Cli::command();
        let help = command.render_long_help().to_string();
        assert!(help.contains("Manage Claude and Codex hooks"));
        assert!(!help.contains("repo-level"));
        assert!(!help.contains("bootstrap"));

        let mut hook_install = Cli::command()
            .find_subcommand_mut("hook")
            .and_then(|hook| hook.find_subcommand_mut("install"))
            .unwrap()
            .clone();
        let hook_install_help = hook_install.render_long_help().to_string();
        assert!(hook_install_help.contains("Assistant target (default: all)"));
        assert!(hook_install_help.contains("Git repository for Codex config, hooks, and AGENTS.md"));
        assert!(!hook_install_help.contains("repo-level"));
    }
}
