mod cmd;
mod optimizer;
mod runner;
mod storage;
mod tool_config;
mod transformer;
mod types;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tkn", about = "Shell proxy for token-optimized AI tool output", version)]
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
    /// Clear all stats and logs
    Clean,
    /// Clean up old log files
    Cleanup,
    /// Analyze recorded outputs to help craft an optimal plugin config
    Optimize {
        #[command(subcommand)]
        action: OptimizeAction,
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
enum OptimizeAction {
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
    /// Run the hook (reads stdin, rewrites command, writes stdout)
    Run,
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
                HookAction::Run => cmd::hook::run(),
            }
            0
        }
        Commands::Clean => {
            cmd::clean::run();
            0
        }
        Commands::Cleanup => {
            cmd::cleanup::run();
            0
        }
        Commands::Optimize { action } => {
            match action {
                OptimizeAction::Scan => cmd::optimize::scan(),
                OptimizeAction::Report { args } => cmd::optimize::report(&args),
            }
            0
        }
        Commands::Plugin { action } => {
            match action {
                PluginAction::Install { url } => cmd::plugin::install(url.as_deref()),
                PluginAction::List => cmd::plugin::list(),
                PluginAction::Remove { name } => cmd::plugin::remove(&name),
            }
            0
        }
    };

    std::process::exit(exit_code);
}
