use clap::Parser;

mod dev;
mod generate;
mod new;

#[derive(Parser)]
#[command(name = "rogrid", version, about = "Scaffold and serve Rojo projects")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    New(new::Args),
    Dev(dev::Args),
    Generate(generate::Args),
}

impl Cli {
    pub fn run(self) -> anyhow::Result<()> {
        match self.command {
            Command::New(args) => new::run(args),
            Command::Dev(args) => dev::run(args),
            Command::Generate(args) => generate::run(args),
        }
    }
}
