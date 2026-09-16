use std::fmt;

use anyhow::Result;
use inquire::Select;

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

/// Asks the user to pick one supported tool. The cursor starts on the first installed one.
pub fn select<T: Tool + Copy>(message: &str, tools: &[T]) -> Result<T> {
    let rows: Vec<Row<T>> = tools
        .iter()
        .filter(|tool| tool.supported())
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
