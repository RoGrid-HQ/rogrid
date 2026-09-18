use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

use crate::commands::{dev, init};

/// Scaffold and manage RoGrid projects for Roblox.
#[derive(Parser)]
#[command(name = "rogrid", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// Every command. Its doc comment is the one-line description shown in help.
#[derive(Subcommand)]
pub enum Command {
    /// Create a new RoGrid project in a new folder, or in the current one with `.`.
    Init(init::Args),
    /// Generate typed event interfaces and watch the project with Rojo.
    Dev(dev::Args),
}

/// Parses the command line. `rogrid help` also lists the tools `init` can set up.
pub fn parse() -> Cli {
    let matches = Cli::command().after_help(init::help::tools()).get_matches();
    Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit())
}
