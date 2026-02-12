mod cmd;
mod optimizer;
mod runner;
mod storage;
mod types;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tkn", about = "Shell proxy for token-optimized AI tool output")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a command through the optimization proxy
    Exec {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Show analytics and usage statistics
    Stats,
    /// Browse and retrieve command logs
    Log {
        /// Reference ID to show full log for
        id: Option<String>,
    },
    /// Install or uninstall the Claude Code hook
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },
    /// Clean up old log files
    Cleanup,
}

#[derive(Subcommand)]
enum HookAction {
    /// Install the Claude Code hook
    Install,
    /// Uninstall the Claude Code hook
    Uninstall,
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Exec { args } => cmd::exec::run(&args),
        Commands::Stats => {
            cmd::stats::run();
            0
        }
        Commands::Log { id } => {
            cmd::logs::run(id.as_deref());
            0
        }
        Commands::Hook { action } => {
            match action {
                HookAction::Install => cmd::hook::install(),
                HookAction::Uninstall => cmd::hook::uninstall(),
            }
            0
        }
        Commands::Cleanup => {
            cmd::cleanup::run();
            0
        }
    };

    std::process::exit(exit_code);
}
