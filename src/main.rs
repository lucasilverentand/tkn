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
    /// Auto-route command to optimized or passthrough execution
    Auto {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Execute a command through the optimization proxy
    Exec {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Pass a command through with inherited stdio (no capture/optimization)
    Pass {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Show analytics and usage statistics
    Stats {
        /// Remove a specific tool from stats (e.g. "git diff")
        #[arg(long)]
        reset: Option<String>,
        /// Reset only the failure count for a tool (e.g. "git diff")
        #[arg(long)]
        reset_failures: Option<String>,
    },
    /// Browse and retrieve command logs
    Log {
        /// Reference ID to show full log for
        id: Option<String>,
        /// Reason for requesting the full log (required when viewing a log)
        reason: Option<String>,
        /// Line range (e.g., "10:20") or single line number (e.g., "42")
        #[arg(long)]
        lines: Option<String>,
    },
    /// Install or uninstall the Claude Code hook
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },
    /// Install or repair assistant integrations
    Setup {
        #[arg(value_enum)]
        target: AssistantTarget,
        /// Repository path for Codex AGENTS.md bootstrap
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Verify assistant integrations and local tkn health
    Doctor {
        #[arg(value_enum)]
        target: Option<AssistantTarget>,
        /// Repository path for Codex AGENTS.md verification
        #[arg(long)]
        repo: Option<PathBuf>,
        /// Emit machine-readable JSON output
        #[arg(long)]
        json: bool,
    },
    /// Clear stats and logs
    Clean {
        /// Only clear logs
        #[arg(long)]
        logs: bool,
        /// Only clear stats
        #[arg(long)]
        stats: bool,
    },
    /// Analyze recorded outputs to help craft an optimal plugin config
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
    /// Install plugins (built-ins or from a git repo URL)
    Install {
        /// Git repository URL (omit to install built-in defaults)
        url: Option<String>,
    },
    /// List installed and available plugins
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
    /// Install the Claude Code hook
    Install,
    /// Uninstall the Claude Code hook
    Uninstall,
    /// Run the Claude Code hook (reads stdin, rewrites command, writes stdout)
    Run {
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
            HookAction::Install => cmd::hook::install(),
            HookAction::Uninstall => cmd::hook::uninstall(),
            HookAction::Run { .. } => {
                cmd::hook::run();
                0
            }
        },
        Commands::Setup { target, repo } => cmd::setup::run(target, repo.as_deref()),
        Commands::Doctor { target, repo, json } => {
            cmd::doctor::run(target, repo.as_deref(), json)
        }
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
