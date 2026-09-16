use clap::builder::PossibleValuesParser;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

use crate::help;
use crate::tools;

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
    Init {
        /// Folder to create, or `.` for the current folder. Prompted for when omitted.
        #[arg(value_name = "FOLDER")]
        target: Option<String>,

        /// Project name. Defaults to the folder name; prompted for with `.`.
        #[arg(long, value_name = "NAME")]
        name: Option<String>,

        /// Set up the project in a folder that is not empty, overwriting files it writes.
        #[arg(long)]
        force: bool,

        /// Package manager to use. Prompted for when omitted.
        #[arg(long, value_name = "NAME", value_parser = PossibleValuesParser::new(tools::names(tools::PACKAGE_MANAGERS)))]
        package_manager: Option<String>,

        /// Tool manager to use. Prompted for when omitted.
        #[arg(long, value_name = "NAME", value_parser = PossibleValuesParser::new(tools::names(tools::TOOL_MANAGERS)))]
        tool_manager: Option<String>,
    },
}

/// Parses the command line. `rogrid help` also lists the tools `init` can set up.
pub fn parse() -> Cli {
    let matches = Cli::command().after_help(help::tools()).get_matches();
    Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit())
}
