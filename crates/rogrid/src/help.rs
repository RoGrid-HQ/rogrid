//! The tool summary appended to `rogrid help`, built from the tools table.

use crate::tools::{self, Tool};

/// Supported package managers and tool managers, with install status and homepage.
pub fn tools() -> String {
    format!(
        "Package managers:\n{}\nTool managers:\n{}",
        list(tools::PACKAGE_MANAGERS),
        list(tools::TOOL_MANAGERS),
    )
}

fn list<T: Tool>(items: &[T]) -> String {
    items
        .iter()
        .filter(|t| t.supported())
        .map(|t| {
            let status = match t.installed() {
                Some(true) => "installed",
                Some(false) => "not found",
                None => "",
            };
            format!("  {:<8} {:<10} {}\n", t.name(), status, t.homepage())
        })
        .collect()
}
