use std::env;

use anyhow::{Context, Result};

use crate::codegen;

/// What the Rojo project needs so the generated code ends up in the game.
pub const MAPPING_HINT: &str = r#"
default.project.json does not map the generated code into the game yet. Add:

  under "ReplicatedStorage":     "RoGrid": { "$path": { "optional": ".rogrid/client" } }
  under "ServerScriptService":   "RoGrid": { "$path": { "optional": ".rogrid/server" } }"#;

pub fn run() -> Result<()> {
    let root = env::current_dir().context("could not read the current directory")?;
    let summary = codegen::run(&root)?;

    println!("Generated {} into .rogrid", requests(summary.count));
    if !summary.mapped {
        println!("{MAPPING_HINT}");
    }

    Ok(())
}

/// `1 request`, `4 requests`.
pub fn requests(count: usize) -> String {
    let noun = if count == 1 { "request" } else { "requests" };
    format!("{count} {noun}")
}
