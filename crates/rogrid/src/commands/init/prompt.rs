use std::fmt;
use std::io::{self, IsTerminal};

use anyhow::{Result, bail};
use inquire::{Select, Text};

use crate::tools::Tool;

/// A tool as shown in a prompt, with whether its binary was found on the PATH.
struct Row<T> {
    tool: T,
    installed: Option<bool>,
}

impl<T: Tool> fmt::Display for Row<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.tool.name();
        match self.installed {
            Some(true) => write!(f, "{name:<8} ✓ installed"),
            Some(false) => write!(f, "{name:<8} ✗ not found"),
            None => write!(f, "{name}"),
        }
    }
}

/// Asks the user for a line of text. Pressing Enter on an empty line takes the default.
pub fn text(message: &str, default: Option<&str>) -> Result<String> {
    ensure_terminal(message)?;
    let mut prompt = Text::new(message);
    if let Some(default) = default {
        prompt = prompt.with_default(default);
    }
    Ok(prompt.prompt()?)
}

/// Asks the user to pick a tool. The cursor starts on the first installed one.
pub fn select<T: Tool + Copy>(message: &str, tools: &[T]) -> Result<T> {
    ensure_terminal(message)?;

    let rows: Vec<Row<T>> = tools
        .iter()
        .map(|&tool| Row {
            tool,
            installed: tool.installed(),
        })
        .collect();

    let start = rows
        .iter()
        .position(|row| row.installed == Some(true))
        .unwrap_or(0);

    let row = Select::new(message, rows)
        .with_starting_cursor(start)
        .prompt()?;

    Ok(row.tool)
}

/// Prompts need a terminal. Without one, say what was needed and how to pass it instead.
fn ensure_terminal(message: &str) -> Result<()> {
    if io::stdin().is_terminal() {
        return Ok(());
    }
    let what = message.trim_end_matches(':').to_lowercase();
    bail!(
        "cannot ask for the {what} without a terminal; pass it as an argument, see `rogrid init --help`"
    );
}
