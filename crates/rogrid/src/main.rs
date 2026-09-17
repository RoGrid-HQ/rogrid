mod binaries;
mod cli;
mod codegen;
mod commands;
mod help;
mod install;
mod process;
mod project;
mod prompt;
mod template;
mod tools;

use cli::Command;

fn main() -> anyhow::Result<()> {
    let cli = cli::parse();

    match cli.command {
        Command::Init {
            target,
            name,
            force,
            package_manager,
            tool_manager,
        } => commands::init::run(target, name, force, package_manager, tool_manager),
        Command::Generate => commands::generate::run(),
        Command::Dev => commands::dev::run(),
    }
}
