//! Everything `rogrid init` knows about package managers and tool managers.
//! Add support for a new one by adding an entry to the matching list.

use anyhow::{Context, Result};

use crate::binaries;
use crate::template;

/// What both lists have in common, so the prompt and the installer can treat them alike.
pub trait Tool {
    fn name(&self) -> &'static str;
    /// Executable to look for on the PATH. `None` when nothing needs installing.
    fn binary(&self) -> Option<&'static str>;
    /// Where a user goes to install the tool itself.
    fn homepage(&self) -> &'static str;
    /// Whether `rogrid init` can set this tool up today. Unsupported tools are not offered.
    fn supported(&self) -> bool;
    /// Commands run in the project folder, in order. `{{placeholders}}` are filled first.
    fn install(&self) -> &'static [&'static str];

    /// Whether the binary is on the PATH. `None` when the tool has no binary.
    fn installed(&self) -> Option<bool> {
        self.binary().map(binaries::is_installed)
    }
}

/// Rojo goes into every project. Same `alias=owner/repo@version` form as `pins` below.
pub const ROJO: &str = "rojo=rojo-rbx/rojo@7.7.0";

#[derive(Clone, Copy)]
pub struct PackageManager {
    pub name: &'static str,
    pub binary: &'static str,
    pub homepage: &'static str,
    pub supported: bool,
    /// Template file copied only when this manager is chosen.
    pub manifest: &'static str,
    pub install: &'static [&'static str],
    /// Where shared and server packages land, relative to the project root.
    pub packages: &'static str,
    pub server_packages: &'static str,
    /// Extra folders for `.gitignore`.
    pub ignores: &'static [&'static str],
    /// Tools a tool manager must install for this manager, as `alias=owner/repo@version`.
    pub pins: &'static [&'static str],
    /// True when the manager pins tools in its own manifest, so no tool manager is asked for.
    pub manages_tools: bool,
}

pub const PACKAGE_MANAGERS: &[PackageManager] = &[
    PackageManager {
        name: "wally",
        binary: "wally",
        homepage: "https://wally.run",
        supported: false,
        manifest: "wally.toml",
        install: &["wally install"],
        packages: "Packages",
        server_packages: "ServerPackages",
        ignores: &[],
        pins: &["wally=UpliftGames/wally@0.3.2"],
        manages_tools: false,
    },
    PackageManager {
        name: "pesde",
        binary: "pesde",
        homepage: "https://docs.pesde.dev/installation",
        supported: true,
        manifest: "pesde.toml",
        install: &["pesde install"],
        packages: "roblox_packages",
        server_packages: "roblox_server_packages",
        ignores: &["/.pesde", "/lune_packages", "/luau_packages"],
        pins: &[
            "pesde=pesde-pkg/pesde@0.7.4+registry.0.2.3",
            "lune=lune-org/lune@0.10.5",
        ],
        manages_tools: false,
    },
    PackageManager {
        name: "ember",
        binary: "embr",
        homepage: "https://luaupm.com/docs/installation",
        supported: false,
        manifest: "ember.toml",
        install: &["embr install"],
        packages: "packages/shared",
        server_packages: "packages/server",
        ignores: &["/.ember-patch"],
        pins: &[],
        manages_tools: true,
    },
];

#[derive(Clone, Copy)]
pub struct ToolManager {
    pub name: &'static str,
    pub binary: Option<&'static str>,
    pub homepage: &'static str,
    pub supported: bool,
    /// File written into the project. Empty when the manager has none.
    pub manifest: &'static str,
    /// One line of that file per pinned tool. Placeholders: alias, spec, repo, version.
    pub pin_line: &'static str,
    pub install: &'static [&'static str],
}

pub const TOOL_MANAGERS: &[ToolManager] = &[
    ToolManager {
        name: "rokit",
        binary: Some("rokit"),
        homepage: "https://github.com/rojo-rbx/rokit",
        supported: true,
        manifest: "rokit.toml",
        pin_line: r#"{{alias}} = "{{spec}}""#,
        install: &["rokit trust {{pin_ids}}", "rokit install"],
    },
    ToolManager {
        name: "aftman",
        binary: Some("aftman"),
        homepage: "https://github.com/LPGhatguy/aftman",
        supported: true,
        manifest: "aftman.toml",
        pin_line: r#"{{alias}} = "{{spec}}""#,
        install: &["aftman trust {{pin_ids}}", "aftman install"],
    },
    ToolManager {
        name: "foreman",
        binary: Some("foreman"),
        homepage: "https://github.com/Roblox/foreman",
        supported: true,
        manifest: "foreman.toml",
        pin_line: r#"{{alias}} = { github = "{{repo}}", version = "{{version}}" }"#,
        install: &["foreman install"],
    },
    ToolManager {
        name: "mise",
        binary: Some("mise"),
        homepage: "https://mise.jdx.dev",
        supported: false,
        manifest: "mise.toml",
        pin_line: r#""ubi:{{repo}}" = "{{version}}""#,
        install: &["mise install"],
    },
    ToolManager {
        name: "none",
        binary: None,
        homepage: "",
        supported: true,
        manifest: "",
        pin_line: "",
        install: &[],
    },
];

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
    fn supported(&self) -> bool {
        self.supported
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
    fn supported(&self) -> bool {
        self.supported
    }
    fn install(&self) -> &'static [&'static str] {
        self.install
    }
}

/// Placeholder values for template files and install commands.
/// `package_name` is the project name made safe for manifests, which do not allow dashes.
pub fn vars(project_name: &str, package_manager: &PackageManager) -> Vec<(&'static str, String)> {
    let pin_ids = pins(package_manager)
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
        ("rojo_pin", pin_var(ROJO, "spec").unwrap_or_default()),
        ("pin_ids", pin_ids),
    ]
}

/// The tool manager's manifest for this package manager: Rojo plus the manager's pins.
pub fn tool_manifest(tool_manager: &ToolManager, package_manager: &PackageManager) -> String {
    let lines = pins(package_manager)
        .filter_map(pin_vars)
        .map(|vars| template::fill(tool_manager.pin_line, &vars))
        .collect::<Vec<_>>()
        .join("\n");

    format!("[tools]\n{lines}\n")
}

/// Names of the supported entries, for flag validation and help.
pub fn names<T: Tool>(items: &[T]) -> Vec<&'static str> {
    items
        .iter()
        .filter(|t| t.supported())
        .map(|t| t.name())
        .collect()
}

/// The supported entry with this name.
pub fn find<T: Tool + Copy>(items: &[T], name: &str) -> Result<T> {
    items
        .iter()
        .find(|t| t.supported() && t.name() == name)
        .copied()
        .with_context(|| format!("unknown tool `{name}`, expected one of {:?}", names(items)))
}

/// Manifest files belonging to the package managers that were not chosen.
pub fn other_manifests(chosen: &PackageManager) -> Vec<&'static str> {
    PACKAGE_MANAGERS
        .iter()
        .filter(|pm| pm.name != chosen.name)
        .map(|pm| pm.manifest)
        .collect()
}

/// Every tool a tool manager installs for this package manager.
fn pins(package_manager: &PackageManager) -> impl Iterator<Item = &'static str> {
    std::iter::once(ROJO).chain(package_manager.pins.iter().copied())
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
