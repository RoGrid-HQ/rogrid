use std::sync::LazyLock;

use anyhow::{Result, ensure};
use fancy_regex::Regex;

use crate::config;

use super::{Manifest, PackageManager};

pub const PACKAGE_MANAGER: PackageManager = PackageManager {
    name: "wally",
    binary: "wally",
    homepage: "https://wally.run/install",
    install: &["wally install"],
    bin_dir: None,
    manifest: Manifest {
        file: "wally.toml",
        template: include_str!("wally.toml"),
    },
    package_name,
    provides_rojo: false,
    packages: "Packages",
    server_packages: "ServerPackages",
    rogrid_dependency: r#"rogrid = "rogrid-hq/rogrid@={{framework_version}}""#,
    ignores: &["/DevPackages"],
    pins: &["wally=UpliftGames/wally@0.3.2"],
};

fn package_name(name: &str) -> Result<String> {
    static RULE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(config::WALLY_PACKAGE_NAME)
            .expect("invalid Wally package-name regex in config.rs")
    });
    let name = name.replace('_', "-");
    ensure!(RULE.is_match(&name)?, config::WALLY_PACKAGE_NAME_ERROR);
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_validates_manifest_names() {
        for (input, expected) in [
            ("a", "a"),
            ("0", "0"),
            ("123", "123"),
            ("my_game-v2", "my-game-v2"),
            ("-", "-"),
            ("_", "-"),
            ("_game_", "-game-"),
            ("a__b", "a--b"),
        ] {
            assert_eq!(package_name(input).unwrap(), expected);
        }
        for name in [
            "",
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
                config::WALLY_PACKAGE_NAME_ERROR,
                "{name:?}"
            );
        }
    }

    #[test]
    fn name_length_matches_the_registry_boundary() {
        for (length, accepted) in [(63, true), (64, true), (65, false)] {
            for character in ["a", "0", "-", "_"] {
                let name = character.repeat(length);
                assert_eq!(package_name(&name).is_ok(), accepted, "{name:?}");
            }
        }
    }
}
