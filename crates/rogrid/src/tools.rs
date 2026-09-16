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
    /// Tools a separate tool manager must install for this manager, as `alias=owner/repo@version`.
    pub pins: &'static [&'static str],
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
    },
];

#[derive(Clone, Copy)]
pub struct ToolManager {
    pub name: &'static str,
    pub binary: Option<&'static str>,
    pub homepage: &'static str,
    pub supported: bool,
    /// Package managers this can pair with. Empty means any.
    pub only_with: &'static [&'static str],
    /// File written into the project. Empty when the tools go into the package
    /// manager's own manifest instead, through its `{{tool_lines}}` placeholder.
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
        supported: true,
        only_with: &[],
        manifest: "rokit.toml",
        pin_line: r#"{{alias}} = "{{spec}}""#,
        install: &["rokit trust {{pin_ids}}", "rokit install"],
    },
    ToolManager {
        name: "aftman",
        binary: Some("aftman"),
        homepage: "https://github.com/LPGhatguy/aftman",
        supported: false,
        only_with: &[],
        manifest: "aftman.toml",
        pin_line: r#"{{alias}} = "{{spec}}""#,
        install: &["aftman trust {{pin_ids}}", "aftman install"],
    },
    ToolManager {
        name: "foreman",
        binary: Some("foreman"),
        homepage: "https://github.com/Roblox/foreman",
        supported: false,
        only_with: &[],
        manifest: "foreman.toml",
        pin_line: r#"{{alias}} = { github = "{{repo}}", version = "{{version}}" }"#,
        install: &["foreman install"],
    },
    ToolManager {
        name: "mise",
        binary: Some("mise"),
        homepage: "https://mise.jdx.dev",
        supported: false,
        only_with: &[],
        manifest: "mise.toml",
        pin_line: r#""ubi:{{repo}}" = "{{version}}""#,
        install: &["mise install"],
    },
    // Tools as dev dependencies from pesde's registry. `pesde install` handles them.
    ToolManager {
        name: "pesde",
        binary: Some("pesde"),
        homepage: "https://docs.pesde.dev/installation",
        supported: true,
        only_with: &["pesde"],
        manifest: "",
        pin_line: r#"{{alias}} = { name = "pesde/{{alias}}", version = "={{version}}", target = "lune" }"#,
        install: &[],
    },
    // Ember pins tools in its own manifest. `embr install` handles them.
    ToolManager {
        name: "ember",
        binary: Some("embr"),
        homepage: "https://luaupm.com/docs/installation",
        supported: false,
        only_with: &["ember"],
        manifest: "",
        pin_line: r#"{{alias}} = "{{spec}}""#,
        install: &[],
    },
    ToolManager {
        name: "none",
        binary: None,
        homepage: "",
        supported: true,
        only_with: &[],
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

/// Supported tool managers that can pair with this package manager.
pub fn tool_managers_for(package_manager: &PackageManager) -> Vec<ToolManager> {
    TOOL_MANAGERS
        .iter()
        .filter(|tm| tm.supported)
        .filter(|tm| tm.only_with.is_empty() || tm.only_with.contains(&package_manager.name))
        .copied()
        .collect()
}

/// Placeholder values for template files and install commands.
/// `package_name` is the project name made safe for manifests, which do not allow dashes.
pub fn vars(
    project_name: &str,
    package_manager: &PackageManager,
    tool_manager: &ToolManager,
) -> Vec<(&'static str, String)> {
    let pin_ids = pins(package_manager, tool_manager)
        .filter_map(|pin| pin_var(pin, "repo"))
        .collect::<Vec<_>>()
        .join(" ");

    // Only for tool managers that live inside the package manager's manifest.
    let tool_lines = if tool_manager.manifest.is_empty() {
        pin_lines(package_manager, tool_manager)
    } else {
        String::new()
    };

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
        ("tool_lines", tool_lines),
    ]
}

/// The tool manager's own manifest file, for tool managers that have one.
pub fn tool_manifest(package_manager: &PackageManager, tool_manager: &ToolManager) -> String {
    format!("[tools]\n{}\n", pin_lines(package_manager, tool_manager))
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

/// One rendered line per pin, in the tool manager's format.
fn pin_lines(package_manager: &PackageManager, tool_manager: &ToolManager) -> String {
    if tool_manager.pin_line.is_empty() {
        return String::new();
    }
    pins(package_manager, tool_manager)
        .filter_map(pin_vars)
        .map(|vars| template::fill(tool_manager.pin_line, &vars))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What this tool manager installs: Rojo, plus the package manager's own pins
/// unless the tool manager is the package manager itself, which already has them.
fn pins(
    package_manager: &PackageManager,
    tool_manager: &ToolManager,
) -> impl Iterator<Item = &'static str> {
    let own = tool_manager.manifest.is_empty();
    std::iter::once(ROJO).chain(package_manager.pins.iter().copied().filter(move |_| !own))
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
