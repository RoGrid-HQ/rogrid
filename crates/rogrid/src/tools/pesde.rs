use anyhow::{Result, ensure};

use super::{Manifest, PackageManager, ToolManager};

pub const PACKAGE_MANAGER: PackageManager = PackageManager {
    name: "pesde",
    binary: "pesde",
    homepage: "https://docs.pesde.dev/installation",
    install: &["pesde install"],
    bin_dir: Some(".pesde/bin"),
    manifest: Manifest {
        file: "pesde.toml",
        template: include_str!("pesde.toml"),
    },
    package_name,
    provides_rojo: true,
    packages: "roblox_packages",
    server_packages: "roblox_server_packages",
    rogrid_dependency: r#"rogrid = { name = "rogrid/rogrid", version = "={{framework_version}}" }"#,
    ignores: &["/.pesde", "/lune_packages", "/luau_packages"],
    pins: &[
        "pesde=pesde-pkg/pesde@0.7.4+registry.0.2.3",
        "lune=lune-org/lune@0.10.5",
    ],
};

// Rojo and the engine shims come from the package installation above.
pub const TOOL_MANAGER: ToolManager = ToolManager {
    name: "pesde",
    binary: Some("pesde"),
    homepage: "https://docs.pesde.dev/installation",
    manifest: None,
    pin_line: "",
    install: &[],
    installs_tools: false,
    only_with: Some("pesde"),
};

fn package_name(name: &str) -> Result<String> {
    let name = name.replace('-', "_");
    ensure!(
        (1..=32).contains(&name.len()),
        "Pesde package names must be 1–32 characters long"
    );
    ensure!(
        name.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
        "Pesde package names may only contain lowercase letters, digits and underscores"
    );
    ensure!(
        !name.starts_with('_') && !name.ends_with('_'),
        "Pesde package names cannot start or end with an underscore or dash"
    );
    ensure!(
        !name.chars().all(|c| c.is_ascii_digit()),
        "Pesde package names cannot contain only digits"
    );
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_validates_manifest_names() {
        assert_eq!(package_name("my-game_v2").unwrap(), "my_game_v2");
        assert!(package_name(&"a".repeat(32)).is_ok());
        for name in ["", "123", "_game", "game-", "Game", "a/b", &"a".repeat(33)] {
            assert!(package_name(name).is_err(), "{name}");
        }
    }
}
