mod commands;
mod tools;

use clap::Parser;
use commands::Cli;

fn main() -> anyhow::Result<()> {
    Cli::parse().run()
}
