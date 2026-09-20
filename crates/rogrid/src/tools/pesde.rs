use std::sync::LazyLock;

use anyhow::{Result, ensure};
use fancy_regex::Regex;

use crate::config;

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
    static RULE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(config::PESDE_PACKAGE_NAME)
            .expect("invalid Pesde package-name regex in config.rs")
    });
    let name = name.replace('-', "_");
    ensure!(RULE.is_match(&name)?, config::PESDE_PACKAGE_NAME_ERROR);
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_validates_manifest_names() {
        for (input, expected) in [
            ("a", "a"),
            ("a0", "a0"),
            ("0a", "0a"),
            ("my-game_v2", "my_game_v2"),
            ("a__b", "a__b"),
            ("1_2", "1_2"),
            ("1-2", "1_2"),
        ] {
            assert_eq!(package_name(input).unwrap(), expected);
        }
        for name in [
            "",
            "0",
            "123",
            "_",
            "-",
            "_game",
            "-game",
            "game_",
            "game-",
            "Game",
            "a/b",
            "a.b",
            "a b",
            "a\tb",
            "a\nb",
            "a\n",
            "\na",
            "a\r\n",
            "a\0b",
            "café",
            "１２３",
        ] {
            assert_eq!(
                package_name(name).unwrap_err().to_string(),
                config::PESDE_PACKAGE_NAME_ERROR,
                "{name:?}"
            );
        }
    }

    #[test]
    fn name_length_matches_the_registry_boundary() {
        for (length, accepted) in [(31, true), (32, true), (33, false)] {
            for name in [
                "a".repeat(length),
                format!("a{}", "0".repeat(length - 1)),
                format!("{}a", "0".repeat(length - 1)),
                format!("1{}2", "_".repeat(length - 2)),
            ] {
                assert_eq!(package_name(&name).is_ok(), accepted, "{name:?}");
            }
        }
        assert!(package_name(&"0".repeat(32)).is_err());
    }
}
