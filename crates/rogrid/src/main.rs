mod cli;
mod codegen;
mod commands;
mod process;
mod template;
mod tools;

use cli::Command;

fn main() -> anyhow::Result<()> {
    let cli = cli::parse();

    match cli.command {
        Command::Init(args) => commands::init::run(args),
        Command::Dev(args) => commands::dev::run(args),
    }
}
