use clap::{Parser, Subcommand};

/// The RoGrid CLI.
#[derive(Parser)]
#[command(name = "rogrid", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a new RoGrid project in a new folder.
    Init {
        /// Project name. Prompted for when omitted.
        name: Option<String>,
    },
}
