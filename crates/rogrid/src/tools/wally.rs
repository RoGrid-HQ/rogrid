use anyhow::{Result, ensure};

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
    let name = name.replace('_', "-");
    ensure!(
        (1..=64).contains(&name.len()),
        "Wally package names must be 1–64 characters long"
    );
    ensure!(
        name.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
        "Wally package names may only contain lowercase letters, digits and dashes"
    );
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_validates_manifest_names() {
        assert_eq!(package_name("my_game-v2").unwrap(), "my-game-v2");
        assert!(package_name("123").is_ok());
        assert!(package_name(&"a".repeat(64)).is_ok());
        for name in ["", "Game", "a/b", &"a".repeat(65)] {
            assert!(package_name(name).is_err(), "{name}");
        }
    }
}
