//! Everything `rogrid init` knows about package managers and tool managers.

use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::process;
use crate::template;

/// What both lists have in common, so the prompt and the installer can treat them alike.
pub trait Tool {
    fn name(&self) -> &'static str;
    /// Executable to look for on the PATH. `None` when nothing needs installing.
    fn binary(&self) -> Option<&'static str>;
    /// Where a user goes to install the tool itself.
    fn homepage(&self) -> &'static str;
    /// Commands run in the project folder, in order. `{{placeholders}}` are filled first.
    fn install(&self) -> &'static [&'static str];

    /// Whether the binary is on the PATH. `None` when the tool has no binary.
    fn installed(&self) -> Option<bool> {
        self.binary().map(process::is_installed)
    }
}

/// Rojo goes into every project. Same `alias=owner/repo@version` form as `pins` below.
pub const ROJO: &str = "rojo=rojo-rbx/rojo@7.7.0";

/// The CLI pins itself at the version that created the project, so everyone
/// working on it runs the same one. Only tool managers with their own manifest
/// can install it: the package managers' registries have no rogrid package.
pub const ROGRID: &str = concat!("rogrid=RoGrid-HQ/rogrid@", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Copy)]
pub struct PackageManager {
    pub name: &'static str,
    pub binary: &'static str,
    pub homepage: &'static str,
    pub install: &'static [&'static str],
    /// Folder under the home directory holding the tools this manager installs.
    /// Searched first while installing, so another tool manager's stand-in for
    /// the same tool cannot take its place.
    pub bin_dir: &'static str,
    /// Where shared and server packages land, relative to the project root.
    pub packages: &'static str,
    pub server_packages: &'static str,
    /// Dependency entry used by the package manager's project template.
    pub rogrid_dependency: &'static str,
    /// Extra folders for `.gitignore`.
    pub ignores: &'static [&'static str],
    /// Tools a separate tool manager must install for this manager, as `alias=owner/repo@version`.
    pub pins: &'static [&'static str],
}

pub const PACKAGE_MANAGERS: &[PackageManager] = &[PackageManager {
    name: "pesde",
    binary: "pesde",
    homepage: "https://docs.pesde.dev/installation",
    install: &["pesde install"],
    bin_dir: ".pesde/bin",
    packages: "roblox_packages",
    server_packages: "roblox_server_packages",
    rogrid_dependency: r#"rogrid = { name = "rogrid/rogrid", version = "=0.2.0" }"#,
    ignores: &["/.pesde", "/lune_packages", "/luau_packages"],
    pins: &[
        "pesde=pesde-pkg/pesde@0.7.4+registry.0.2.3",
        "lune=lune-org/lune@0.10.5",
    ],
}];

#[derive(Clone, Copy)]
pub struct ToolManager {
    pub name: &'static str,
    pub binary: Option<&'static str>,
    pub homepage: &'static str,
    /// File written into the project. Empty when no separate tool manifest is needed.
    pub manifest: &'static str,
    /// One line per pinned tool. Placeholders: alias, spec, repo, version.
    pub pin_line: &'static str,
    pub install: &'static [&'static str],
}

pub const TOOL_MANAGERS: &[ToolManager] = &[
    ToolManager {
        name: "rokit",
        binary: Some("rokit"),
        homepage: "https://github.com/rojo-rbx/rokit",
        manifest: "rokit.toml",
        pin_line: r#"{{alias}} = "{{spec}}""#,
        install: &["rokit trust {{pin_ids}}", "rokit install"],
    },
    // Tools as dev dependencies from pesde's registry. `pesde install` handles them.
    ToolManager {
        name: "pesde",
        binary: Some("pesde"),
        homepage: "https://docs.pesde.dev/installation",
        manifest: "",
        pin_line: "",
        install: &[],
    },
    ToolManager {
        name: "none",
        binary: None,
        homepage: "",
        manifest: "",
        pin_line: "",
        install: &[],
    },
];

impl PackageManager {
    /// The full path of `bin_dir`. `None` when the home directory is unknown.
    pub fn bin_path(&self) -> Option<PathBuf> {
        env::home_dir().map(|home| home.join(self.bin_dir))
    }
}

impl Tool for PackageManager {
    fn name(&self) -> &'static str {
        self.name
    }
    fn binary(&self) -> Option<&'static str> {
        Some(self.binary)
    }
    fn homepage(&self) -> &'static str {
        self.homepage
    }
    fn install(&self) -> &'static [&'static str] {
        self.install
    }
}

impl Tool for ToolManager {
    fn name(&self) -> &'static str {
        self.name
    }
    fn binary(&self) -> Option<&'static str> {
        self.binary
    }
    fn homepage(&self) -> &'static str {
        self.homepage
    }
    fn install(&self) -> &'static [&'static str] {
        self.install
    }
}

/// Placeholder values for template files and install commands.
/// `package_name` is the project name made safe for manifests, which do not allow dashes.
/// Local framework setups omit both the registry dependency and released CLI pin.
pub fn vars(
    project_name: &str,
    package_manager: &PackageManager,
    tool_manager: &ToolManager,
    install_rogrid: bool,
) -> Vec<(&'static str, String)> {
    let pin_ids = pins(package_manager, tool_manager, install_rogrid)
        .filter_map(|pin| pin_var(pin, "repo"))
        .collect::<Vec<_>>()
        .join(" ");

    vec![
        ("project_name", project_name.to_string()),
        ("package_name", project_name.replace('-', "_")),
        ("packages_dir", package_manager.packages.to_string()),
        (
            "server_packages_dir",
            package_manager.server_packages.to_string(),
        ),
        ("ignores", package_manager.ignores.join("\n")),
        ("pin_ids", pin_ids),
        (
            "rojo_version",
            pin_var(ROJO, "version").expect("ROJO must contain a version"),
        ),
        (
            "rogrid_dependency",
            if install_rogrid {
                package_manager.rogrid_dependency.to_string()
            } else {
                "# RoGrid is mapped from local source in default.project.json.".to_string()
            },
        ),
    ]
}

/// The tool manager's own manifest file, for tool managers that have one.
pub fn tool_manifest(
    package_manager: &PackageManager,
    tool_manager: &ToolManager,
    install_rogrid: bool,
) -> String {
    format!(
        "[tools]\n{}\n",
        pin_lines(package_manager, tool_manager, install_rogrid)
    )
}

/// Names of the supported entries, for flag validation and help.
pub fn names<T: Tool>(items: &[T]) -> Vec<&'static str> {
    items.iter().map(|t| t.name()).collect()
}

/// The supported entry with this name.
pub fn find<T: Tool + Copy>(items: &[T], name: &str) -> Result<T> {
    items
        .iter()
        .find(|t| t.name() == name)
        .copied()
        .with_context(|| format!("unknown tool `{name}`, expected one of {:?}", names(items)))
}

/// One rendered line per pin, in the tool manager's format.
fn pin_lines(
    package_manager: &PackageManager,
    tool_manager: &ToolManager,
    install_rogrid: bool,
) -> String {
    if tool_manager.pin_line.is_empty() {
        return String::new();
    }
    pins(package_manager, tool_manager, install_rogrid)
        .filter_map(pin_vars)
        .map(|vars| template::fill(tool_manager.pin_line, &vars))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What this tool manager installs: Rojo, plus the CLI and the package
/// manager's own pins unless the tool manager is the package manager itself,
/// which already has itself and cannot install the CLI.
fn pins(
    package_manager: &PackageManager,
    tool_manager: &ToolManager,
    install_rogrid: bool,
) -> impl Iterator<Item = &'static str> {
    let own = tool_manager.manifest.is_empty();
    let extra = std::iter::once(ROGRID)
        .filter(move |_| install_rogrid)
        .chain(package_manager.pins.iter().copied());
    std::iter::once(ROJO).chain(extra.filter(move |_| !own))
}

/// Splits `alias=owner/repo@version` into alias, spec, repo and version placeholders.
fn pin_vars(pin: &'static str) -> Option<Vec<(&'static str, String)>> {
    let (alias, spec) = pin.split_once('=')?;
    let (repo, version) = spec.split_once('@')?;
    Some(vec![
        ("alias", alias.to_string()),
        ("spec", spec.to_string()),
        ("repo", repo.to_string()),
        ("version", version.to_string()),
    ])
}

/// One placeholder value from a pin.
fn pin_var(pin: &'static str, key: &str) -> Option<String> {
    pin_vars(pin)?
        .into_iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
}
