//! Everything `rogrid init` knows about package managers and tool managers.

mod pesde;
mod rokit;
mod wally;

use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

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
/// can install it: the package managers' registries do not supply the CLI binary.
pub const ROGRID: &str = concat!("rogrid=RoGrid-HQ/rogrid@", env!("CARGO_PKG_VERSION"));

/// Runtime release expected by this CLI, independent of the CLI's own version.
pub const FRAMEWORK_VERSION: &str = "0.2.2";

#[derive(Clone, Copy)]
pub struct Manifest {
    pub file: &'static str,
    pub template: &'static str,
}

#[derive(Clone, Copy)]
pub struct PackageManager {
    pub name: &'static str,
    pub binary: &'static str,
    pub homepage: &'static str,
    pub install: &'static [&'static str],
    /// Folder under the home directory holding the tools this manager installs.
    /// Searched first while installing, so another tool manager's stand-in for
    /// the same tool cannot take its place.
    pub bin_dir: Option<&'static str>,
    pub manifest: Manifest,
    /// Normalize and validate the name written to this manager's manifest.
    pub package_name: fn(&str) -> Result<String>,
    /// Whether package installation also supplies a Rojo executable.
    pub provides_rojo: bool,
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

pub const PACKAGE_MANAGERS: &[PackageManager] = &[pesde::PACKAGE_MANAGER, wally::PACKAGE_MANAGER];

#[derive(Clone, Copy)]
pub struct ToolManager {
    pub name: &'static str,
    pub binary: Option<&'static str>,
    pub homepage: &'static str,
    pub manifest: Option<Manifest>,
    /// One line per pinned tool. Placeholders: alias, spec, repo, version.
    pub pin_line: &'static str,
    pub install: &'static [&'static str],
    /// This manager provisions the package manager and Rojo from their tool pins.
    pub installs_tools: bool,
    /// A tool manager that relies on a particular package manager's installation.
    pub only_with: Option<&'static str>,
}

pub const TOOL_MANAGERS: &[ToolManager] = &[
    rokit::TOOL_MANAGER,
    pesde::TOOL_MANAGER,
    ToolManager {
        name: "none",
        binary: None,
        homepage: "",
        manifest: None,
        pin_line: "",
        install: &[],
        installs_tools: false,
        only_with: None,
    },
];

impl PackageManager {
    /// The full path of `bin_dir`. `None` when the home directory is unknown.
    pub fn bin_path(&self) -> Option<PathBuf> {
        self.bin_dir
            .and_then(|dir| env::home_dir().map(|home| home.join(dir)))
    }
}

impl ToolManager {
    pub fn supports(&self, package_manager: &PackageManager) -> bool {
        self.only_with
            .is_none_or(|name| name == package_manager.name)
    }
}

/// Rojo ownership controls executable lookup and the advice given after setup.
#[derive(Debug, PartialEq, Eq)]
pub enum RojoFrom {
    ToolManager,
    PackageManager,
    Path,
}

pub struct InstallStep {
    pub tool: &'static str,
    pub homepage: &'static str,
    pub command: String,
}

pub struct Setup {
    pub vars: Vec<(&'static str, String)>,
    pub files: Vec<(&'static str, String)>,
    pub steps: Vec<InstallStep>,
    pub path_first: Vec<PathBuf>,
    pub rojo: RojoFrom,
}

impl Setup {
    pub fn rojo_path(&self) -> &[PathBuf] {
        match self.rojo {
            RojoFrom::PackageManager => &self.path_first,
            _ => &[],
        }
    }
}

/// Resolve and validate setup before creating files or running installers.
pub fn setup(
    name: &str,
    pm: &PackageManager,
    tm: &ToolManager,
    install_rogrid: bool,
) -> Result<Setup> {
    if !tm.supports(pm) {
        bail!(
            "{} cannot be used with tool manager {}; supported tool managers: {}",
            pm.name,
            tm.name,
            TOOL_MANAGERS
                .iter()
                .filter(|tool| tool.supports(pm))
                .map(|tool| tool.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let vars = vars(name, pm, tm, install_rogrid)?;
    let files = std::iter::once(pm.manifest)
        .chain(tm.manifest)
        .map(|manifest| (manifest.file, template::fill(manifest.template, &vars)))
        .collect();
    let tools: [&dyn Tool; 2] = [tm, pm];
    let steps = tools
        .into_iter()
        .flat_map(|tool| {
            tool.install()
                .iter()
                .map(|command| InstallStep {
                    tool: tool.name(),
                    homepage: tool.homepage(),
                    command: template::fill(command, &vars),
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let rojo = if tm.installs_tools {
        RojoFrom::ToolManager
    } else if pm.provides_rojo {
        RojoFrom::PackageManager
    } else {
        RojoFrom::Path
    };
    Ok(Setup {
        vars,
        files,
        steps,
        path_first: pm.bin_path().into_iter().collect(),
        rojo,
    })
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
/// Local framework setups omit both the registry dependency and released CLI pin.
fn vars(
    project_name: &str,
    package_manager: &PackageManager,
    tool_manager: &ToolManager,
    install_rogrid: bool,
) -> Result<Vec<(&'static str, String)>> {
    let pin_ids = pins(package_manager, install_rogrid)
        .filter_map(|pin| pin_var(pin, "repo"))
        .collect::<Vec<_>>()
        .join(" ");

    let dependency = template::fill(
        package_manager.rogrid_dependency,
        &[("framework_version", FRAMEWORK_VERSION.to_string())],
    );
    Ok(vec![
        ("project_name", project_name.to_string()),
        (
            "package_name",
            (package_manager.package_name)(project_name)?,
        ),
        ("packages_dir", package_manager.packages.to_string()),
        (
            "server_packages_dir",
            package_manager.server_packages.to_string(),
        ),
        ("ignores", package_manager.ignores.join("\n")),
        ("pin_ids", pin_ids),
        (
            "pins",
            pin_lines(package_manager, tool_manager, install_rogrid),
        ),
        (
            "rojo_version",
            pin_var(ROJO, "version").expect("ROJO must contain a version"),
        ),
        (
            "rogrid_dependency",
            if install_rogrid {
                dependency
            } else {
                "# RoGrid is mapped from local source in default.project.json.".to_string()
            },
        ),
    ])
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
    pins(package_manager, install_rogrid)
        .filter_map(pin_vars)
        .map(|vars| template::fill(tool_manager.pin_line, &vars))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Tools needed by the starter. Local framework work omits the released CLI.
fn pins(
    package_manager: &PackageManager,
    install_rogrid: bool,
) -> impl Iterator<Item = &'static str> {
    let extra = std::iter::once(ROGRID)
        .filter(move |_| install_rogrid)
        .chain(package_manager.pins.iter().copied());
    std::iter::once(ROJO).chain(extra)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pesde_starters_preserve_dependencies_and_tool_choices() {
        let pm = find(PACKAGE_MANAGERS, "pesde").unwrap();
        for tm in TOOL_MANAGERS {
            for released in [false, true] {
                let setup = setup("my-game", &pm, tm, released).unwrap();
                let project = tempfile::tempdir().unwrap();
                template::render(&template::DEFAULT, project.path(), &setup.vars).unwrap();
                for (file, contents) in &setup.files {
                    std::fs::write(project.path().join(file), contents).unwrap();
                }
                let manifest: toml::Value = toml::from_str(
                    &std::fs::read_to_string(project.path().join("pesde.toml")).unwrap(),
                )
                .unwrap();
                assert_eq!(manifest["name"].as_str(), Some("local/my_game"));
                assert_eq!(manifest["dependencies"].get("rogrid").is_some(), released);
                if released {
                    assert_eq!(
                        manifest["dependencies"]["rogrid"]["version"].as_str(),
                        Some(format!("={FRAMEWORK_VERSION}").as_str())
                    );
                }
                assert_eq!(
                    manifest["dev_dependencies"]["rojo"]["version"].as_str(),
                    Some("=7.7.0")
                );
                assert!(manifest["dev_dependencies"].get("scripts_rojo").is_some());
                let rojo: serde_json::Value = serde_json::from_str(
                    &std::fs::read_to_string(project.path().join("default.project.json")).unwrap(),
                )
                .unwrap();
                assert_eq!(
                    rojo["tree"]["ReplicatedStorage"]["Packages"]["$path"]["optional"],
                    "roblox_packages"
                );
                if tm.name == "rokit" {
                    let tools: toml::Value = toml::from_str(
                        &std::fs::read_to_string(project.path().join("rokit.toml")).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(tools["tools"].get("rogrid").is_some(), released);
                    assert_eq!(tools["tools"]["rojo"].as_str(), Some("rojo-rbx/rojo@7.7.0"));
                    assert!(tools["tools"].get("pesde").is_some());
                    assert!(tools["tools"].get("lune").is_some());
                } else {
                    assert!(tm.install.is_empty());
                    assert!(tm.manifest.is_none());
                }
            }
        }
    }

    #[test]
    fn compatibility_matches_the_choices_offered_to_users() {
        let pm = wally::PACKAGE_MANAGER;
        let choices: Vec<_> = TOOL_MANAGERS
            .iter()
            .filter(|tm| tm.supports(&pm))
            .map(|tm| tm.name)
            .collect();
        assert_eq!(choices, ["rokit", "none"]);
        for tm in TOOL_MANAGERS {
            assert_eq!(setup("game", &pm, tm, true).is_ok(), tm.supports(&pm));
        }
        let none = find(TOOL_MANAGERS, "none").unwrap();
        assert_eq!(
            setup("game", &pm, &none, true).unwrap().rojo,
            RojoFrom::Path
        );
        assert_eq!(
            setup("game", &pesde::PACKAGE_MANAGER, &none, true)
                .unwrap()
                .rojo,
            RojoFrom::PackageManager
        );
    }

    #[test]
    fn framework_release_metadata_and_dependency_pins_agree() {
        let pesde: toml::Value =
            toml::from_str(include_str!("../../../../packages/rogrid/pesde.toml")).unwrap();
        let wally: toml::Value =
            toml::from_str(include_str!("../../../../packages/rogrid/wally.toml")).unwrap();
        assert_eq!(pesde["version"].as_str(), Some(FRAMEWORK_VERSION));
        assert_eq!(
            wally["package"]["version"].as_str(),
            Some(FRAMEWORK_VERSION)
        );
        let runtime = include_str!("../../../../packages/rogrid/src/init.luau");
        assert!(runtime.contains(&format!("RoGrid.version = \"{FRAMEWORK_VERSION}\"")));
        let none = find(TOOL_MANAGERS, "none").unwrap();
        for (pm, package) in [
            (pesde::PACKAGE_MANAGER, &pesde),
            (wally::PACKAGE_MANAGER, &wally["package"]),
        ] {
            let setup = setup("game", &pm, &none, true).unwrap();
            let manifest: toml::Value = toml::from_str(&setup.files[0].1).unwrap();
            let dependency = &manifest["dependencies"]["rogrid"];
            if let Some(spec) = dependency.as_str() {
                assert_eq!(
                    spec,
                    format!("{}@={FRAMEWORK_VERSION}", package["name"].as_str().unwrap())
                );
            } else {
                assert_eq!(dependency["name"], package["name"]);
                assert_eq!(
                    dependency["version"].as_str(),
                    Some(format!("={FRAMEWORK_VERSION}").as_str())
                );
            }
        }
    }
}
