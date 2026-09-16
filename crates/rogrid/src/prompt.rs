use std::fmt;

use anyhow::Result;
use inquire::Select;

use crate::binaries;
use crate::tools::Tool;

/// A tool as shown in a prompt, with whether its binary was found on the PATH.
struct Choice {
    tool: Tool,
    installed: Option<bool>,
}

impl fmt::Display for Choice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.tool.supported {
            return write!(f, "{:<8} (soon)", self.tool.name);
        }
        match self.installed {
            Some(true) => write!(f, "{:<8} ✓ installed", self.tool.name),
            Some(false) => write!(f, "{:<8} ✗ not found", self.tool.name),
            None => write!(f, "{}", self.tool.name),
        }
    }
}

/// Asks the user to pick one tool. Unsupported tools are shown but cannot be chosen.
/// The cursor starts on the first supported, installed one.
pub fn select(message: &str, tools: &[Tool]) -> Result<Tool> {
    let start = tools
        .iter()
        .position(|t| t.supported && t.binary.is_some_and(binaries::is_installed))
        .or_else(|| tools.iter().position(|t| t.supported))
        .unwrap_or(0);

    loop {
        let choices: Vec<Choice> = tools
            .iter()
            .map(|&tool| Choice {
                tool,
                installed: tool.binary.map(binaries::is_installed),
            })
            .collect();

        let choice = Select::new(message, choices)
            .with_starting_cursor(start)
            .prompt()?;

        if choice.tool.supported {
            return Ok(choice.tool);
        }
        println!("{} support is coming soon, pick another", choice.tool.name);
    }
}
