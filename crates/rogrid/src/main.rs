mod binaries;
mod cli;
mod commands;
mod install;
mod process;
mod prompt;
mod template;
mod tools;

use clap::Parser;
use cli::{Cli, Command};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init { name } => commands::init::run(name),
    }
}
