mod binaries;
mod cli;
mod commands;
mod help;
mod install;
mod process;
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
    }
}
