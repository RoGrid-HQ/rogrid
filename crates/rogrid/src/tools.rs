use anyhow::{Context, Result};

/// A CLI tool a project can rely on. Each tool is defined once and reused by the lists below.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Tool {
    pub name: &'static str,
    /// Executable to look for on the PATH. `None` when nothing needs installing.
    pub binary: Option<&'static str>,
    /// Template files that belong to this tool. Only copied when the tool is selected.
    pub files: &'static [&'static str],
    /// GitHub release to pin, as `owner/repo@version`. `None` when it cannot be pinned.
    pub pin: Option<&'static str>,
    /// Where this tool installs packages, for package managers only.
    pub packages: Option<Packages>,
    /// Whether `rogrid init` can set this tool up today. Unsupported tools show as "soon".
    pub supported: bool,
}

/// Folders a package manager installs into, relative to the project root.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Packages {
    pub shared: &'static str,
    pub server: &'static str,
}

pub const WALLY: Tool = Tool {
    name: "wally",
    supported: false,
    binary: Some("wally"),
    files: &["wally.toml"],
    pin: Some("UpliftGames/wally@0.3.2"),
    packages: Some(Packages {
        shared: "Packages",
        server: "ServerPackages",
    }),
};

pub const PESDE: Tool = Tool {
    name: "pesde",
    supported: true,
    binary: Some("pesde"),
    files: &["pesde.toml"],
    pin: Some("pesde-pkg/pesde@0.7.4+registry.0.2.3"),
    packages: Some(Packages {
        shared: "roblox_packages",
        server: "roblox_server_packages",
    }),
};

pub const EMBER: Tool = Tool {
    name: "ember",
    supported: false,
    binary: Some("embr"),
    files: &["ember.toml"],
    pin: None,
    packages: Some(Packages {
        shared: "packages/shared",
        server: "packages/server",
    }),
};

pub const ROKIT: Tool = Tool {
    name: "rokit",
    supported: true,
    binary: Some("rokit"),
    files: &["rokit.toml"],
    pin: None,
    packages: None,
};

pub const NONE: Tool = Tool {
    name: "none",
    supported: true,
    binary: None,
    files: &[],
    pin: None,
    packages: None,
};

/// Not a choice, but every project needs it.
pub const ROJO: Tool = Tool {
    name: "rojo",
    supported: true,
    binary: Some("rojo"),
    files: &[],
    pin: Some("rojo-rbx/rojo@7.7.0"),
    packages: None,
};

pub const ALL: &[Tool] = &[WALLY, PESDE, EMBER, ROKIT, NONE];
pub const PACKAGE_MANAGERS: &[Tool] = &[WALLY, PESDE, EMBER];

/// Ember pins tools in its own manifest, so it only shows up here when it is the package manager.
pub fn tool_managers(package_manager: Tool) -> Vec<Tool> {
    let mut list = vec![ROKIT];
    if package_manager == EMBER {
        list.push(EMBER);
    }
    list.push(NONE);
    list
}

/// Template files owned by tools that were not selected.
pub fn unused_files(selected: &[Tool]) -> Vec<&'static str> {
    ALL.iter()
        .filter(|tool| !selected.contains(tool))
        .flat_map(|tool| tool.files.iter().copied())
        .collect()
}

/// Placeholder values for the template, derived from the chosen package manager.
pub fn template_vars(project_name: &str, package_manager: Tool) -> Result<Vec<(&str, String)>> {
    let packages = package_manager
        .packages
        .with_context(|| format!("{} is not a package manager", package_manager.name))?;

    Ok(vec![
        ("project_name", project_name.to_string()),
        ("packages_dir", packages.shared.to_string()),
        ("server_packages_dir", packages.server.to_string()),
        ("rojo_pin", ROJO.pin.unwrap_or_default().to_string()),
        ("package_manager_pin", pin_line(package_manager)),
    ])
}

/// A `name = "owner/repo@version"` line for a tools manifest, or empty when unpinnable.
fn pin_line(tool: Tool) -> String {
    match (tool.binary, tool.pin) {
        (Some(binary), Some(pin)) => format!("{binary} = \"{pin}\""),
        _ => String::new(),
    }
}
