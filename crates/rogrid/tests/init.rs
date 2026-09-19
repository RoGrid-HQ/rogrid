mod support;

use std::fs;

use support::{Sandbox, failure, success, text};

fn initializes(pm: &str, tm: &str, local: bool) {
    let sandbox = Sandbox::new();
    if tm == "rokit" {
        sandbox.provide("rokit");
    } else {
        sandbox.provide(pm);
        // For Pesde this also checks that codegen prefers its freshly installed
        // Rojo over another executable already on PATH.
        sandbox.provide("rojo");
    }
    let mut command = sandbox.command(pm, tm);
    command.args(["my-game", "--name", "my_game-v2"]);
    if local {
        command
            .arg("--local-framework")
            .arg(&sandbox.local_framework);
    }
    let stdout = success(command.output().unwrap());
    assert!(stdout.contains(&format!("({pm} + {tm})")));
    assert_eq!(stdout.contains("Framework source:"), local);
    let project = sandbox.work.join("my-game");
    let is_wally = pm == "wally";
    let (manifest_file, other, packages, server_packages) = if is_wally {
        ("wally.toml", "pesde.toml", "Packages", "ServerPackages")
    } else {
        (
            "pesde.toml",
            "wally.toml",
            "roblox_packages",
            "roblox_server_packages",
        )
    };
    assert!(!project.join(other).exists());
    let manifest_text = text(project.join(manifest_file));
    assert!(!manifest_text.contains("{{"));
    let manifest: toml::Value = toml::from_str(&manifest_text).unwrap();
    let metadata = if is_wally {
        &manifest["package"]
    } else {
        &manifest
    };
    assert_eq!(metadata["private"].as_bool(), Some(true));
    assert_eq!(
        metadata["name"].as_str(),
        Some(if is_wally {
            "local/my-game-v2"
        } else {
            "local/my_game_v2"
        })
    );
    assert_eq!(manifest["dependencies"].get("rogrid").is_some(), !local);
    if !local {
        let runtime: toml::Value =
            toml::from_str(include_str!("../../../packages/rogrid/pesde.toml")).unwrap();
        let version = runtime["version"].as_str().unwrap();
        if is_wally {
            assert_eq!(
                manifest["dependencies"]["rogrid"].as_str(),
                Some(format!("rogrid-hq/rogrid@={version}").as_str())
            );
        } else {
            assert_eq!(
                manifest["dependencies"]["rogrid"]["version"].as_str(),
                Some(format!("={version}").as_str())
            );
        }
    }
    if is_wally {
        assert_eq!(metadata["realm"].as_str(), Some("shared"));
        assert_eq!(
            manifest["place"]["shared-packages"].as_str(),
            Some("game.ReplicatedStorage.Packages")
        );
        assert_eq!(
            manifest["place"]["server-packages"].as_str(),
            Some("game.ServerScriptService.ServerPackages")
        );
        assert!(manifest.get("dev_dependencies").is_none());
        assert!(manifest.get("engines").is_none());
    } else {
        assert_eq!(
            manifest["dev_dependencies"]["rojo"]["version"].as_str(),
            Some("=7.7.0")
        );
        assert!(
            manifest["scripts"]
                .get("roblox_sync_config_generator")
                .is_some()
        );
    }
    let config: serde_json::Value =
        serde_json::from_str(&text(project.join("default.project.json"))).unwrap();
    let shared = &config["tree"]["ReplicatedStorage"]["Packages"];
    assert_eq!(shared["$path"]["optional"], packages);
    assert_eq!(
        config["tree"]["ServerScriptService"]["ServerPackages"]["$path"]["optional"],
        server_packages
    );
    assert_eq!(shared.get("rogrid").is_some(), local);
    if local {
        assert_eq!(
            shared["rogrid"]["$path"].as_str(),
            sandbox.local_framework.join("src").to_str()
        );
    }
    let ignores = text(project.join(".gitignore"));
    assert!(ignores.lines().any(|line| line == format!("/{packages}")));
    assert!(
        ignores
            .lines()
            .any(|line| line == format!("/{server_packages}"))
    );
    assert_eq!(ignores.contains("/DevPackages"), is_wally);
    assert!(!ignores.contains(".lock"));
    assert!(project.join(".vscode/settings.json").is_file());
    assert!(project.join("src/server/events/Lobby.luau").is_file());
    assert!(text(project.join(".rogrid/generated/shared/Server.luau")).contains("setReady"));
    assert!(text(project.join(".rogrid/generated/server/Client.luau")).contains("fireAll"));
    assert!(!text(project.join(".rogrid/generated/shared/Revision.luau")).contains("invalid"));
    assert!(project.join(".rogrid/events.json").is_file());

    let mut expected = Vec::new();
    if tm == "rokit" {
        let pins: toml::Value = toml::from_str(&text(project.join("rokit.toml"))).unwrap();
        assert_eq!(pins["tools"]["rojo"].as_str(), Some("rojo-rbx/rojo@7.7.0"));
        assert_eq!(pins["tools"].get("rogrid").is_some(), !local);
        assert!(pins["tools"].get(pm).is_some());
        assert_eq!(pins["tools"].get("lune").is_some(), !is_wally);
        assert!(
            pins["tools"]
                .get(if is_wally { "pesde" } else { "wally" })
                .is_none()
        );
        let mut trust = "rokit trust rojo-rbx/rojo".to_string();
        if !local {
            trust.push_str(" RoGrid-HQ/rogrid");
        }
        trust.push_str(if is_wally {
            " UpliftGames/wally"
        } else {
            " pesde-pkg/pesde lune-org/lune"
        });
        expected.extend([
            "rokit --version".to_string(),
            trust,
            "rokit install".to_string(),
        ]);
    } else {
        assert!(!project.join("rokit.toml").exists());
        expected.push(format!("{pm} --version"));
        if is_wally {
            expected.push("rojo --version".to_string());
        }
    }
    expected.extend([
        format!("{pm} install"),
        "rojo sourcemap default.project.json".to_string(),
        "rojo --version".to_string(),
    ]);
    assert_eq!(sandbox.commands(), expected);
    let log = sandbox.log();
    let (_, rojo_exe) = log
        .iter()
        .find(|(cmd, _)| cmd.starts_with("rojo sourcemap"))
        .unwrap();
    let expected_dir = if !is_wally && tm != "rokit" {
        sandbox.home.join(".pesde/bin")
    } else {
        sandbox.bin.clone()
    };
    assert_eq!(
        fs::canonicalize(rojo_exe.parent().unwrap()).unwrap(),
        fs::canonicalize(expected_dir).unwrap()
    );
}

#[test]
fn pesde_rokit() {
    initializes("pesde", "rokit", false);
}
#[test]
fn pesde_rokit_local() {
    initializes("pesde", "rokit", true);
}
#[test]
fn pesde_pesde() {
    initializes("pesde", "pesde", false);
}
#[test]
fn pesde_pesde_local() {
    initializes("pesde", "pesde", true);
}
#[test]
fn pesde_none() {
    initializes("pesde", "none", false);
}
#[test]
fn pesde_none_local() {
    initializes("pesde", "none", true);
}
#[test]
fn wally_rokit() {
    initializes("wally", "rokit", false);
}
#[test]
fn wally_rokit_local() {
    initializes("wally", "rokit", true);
}
#[test]
fn wally_none() {
    initializes("wally", "none", false);
}
#[test]
fn wally_none_local() {
    initializes("wally", "none", true);
}

#[test]
fn unsupported_pair_fails_before_writes_or_commands() {
    let sandbox = Sandbox::new();
    failure(
        sandbox
            .command("wally", "pesde")
            .arg("game")
            .output()
            .unwrap(),
        "supported tool managers: rokit, none",
    );
    assert!(!sandbox.work.join("game").exists());
    assert!(sandbox.commands().is_empty());
}

#[test]
fn missing_external_tools_fail_before_writes() {
    for (pm, tm, supplied, message) in [
        ("wally", "none", None, "wally --version"),
        ("wally", "none", Some("wally"), "requires Rojo on PATH"),
        ("wally", "rokit", None, "rokit --version"),
        ("pesde", "none", None, "pesde --version"),
        ("pesde", "pesde", None, "pesde --version"),
    ] {
        let sandbox = Sandbox::new();
        if let Some(tool) = supplied {
            sandbox.provide(tool);
        }
        failure(
            sandbox.command(pm, tm).arg("game").output().unwrap(),
            message,
        );
        assert!(!sandbox.work.join("game").exists());
    }
}

#[test]
fn failed_install_stops_and_preserves_recovery_instructions() {
    let sandbox = Sandbox::new();
    sandbox.provide("rokit");
    let output = sandbox
        .command("wally", "rokit")
        .arg("game")
        .env("ROGRID_TEST_FAIL", "rokit install")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    failure(output, "installation is incomplete");
    let lines: Vec<_> = stderr.lines().map(str::trim).collect();
    assert!(
        lines
            .windows(2)
            .any(|lines| lines == ["rokit install", "wally install"]),
        "{stderr}"
    );
    assert!(
        !sandbox
            .commands()
            .iter()
            .any(|cmd| cmd.starts_with("wally ") || cmd.starts_with("rojo "))
    );
    assert!(
        text(
            sandbox
                .work
                .join("game/.rogrid/generated/shared/Revision.luau")
        )
        .contains("invalid")
    );
}

#[test]
fn failed_package_install_skips_codegen() {
    let sandbox = Sandbox::new();
    sandbox.provide("rokit");
    failure(
        sandbox
            .command("wally", "rokit")
            .arg("game")
            .env("ROGRID_TEST_FAIL", "wally install")
            .output()
            .unwrap(),
        "installation stopped at `wally install`",
    );
    assert!(
        !sandbox
            .commands()
            .iter()
            .any(|cmd| cmd.starts_with("rojo sourcemap"))
    );
}

#[test]
fn failed_codegen_keeps_project_invalid_and_reports_next_step() {
    let sandbox = Sandbox::new();
    sandbox.provide("rokit");
    failure(
        sandbox
            .command("wally", "rokit")
            .arg("game")
            .env("ROGRID_TEST_FAIL", "rojo sourcemap default.project.json")
            .output()
            .unwrap(),
        "run `rogrid dev --once`",
    );
    assert!(
        text(
            sandbox
                .work
                .join("game/.rogrid/generated/shared/Revision.luau")
        )
        .contains("invalid")
    );
}

#[test]
fn current_directory_preserves_git_metadata_without_force() {
    for pm in ["pesde", "wally"] {
        for is_directory in [false, true] {
            let sandbox = Sandbox::new();
            sandbox.provide("rokit");
            let git = sandbox.work.join(".git");
            let metadata = if is_directory {
                git.clone()
            } else {
                sandbox.root.path().join("worktree-metadata")
            };
            fs::create_dir(&metadata).unwrap();
            fs::write(metadata.join("HEAD"), "ref: refs/heads/main\n").unwrap();
            fs::write(metadata.join("config"), "[core]\n\tbare = false\n").unwrap();
            let pointer = format!("gitdir: {}\n", metadata.display());
            if !is_directory {
                fs::write(&git, &pointer).unwrap();
            }

            success(
                sandbox
                    .command(pm, "rokit")
                    .args([".", "--name", "my-game"])
                    .output()
                    .unwrap(),
            );

            assert_eq!(text(metadata.join("HEAD")), "ref: refs/heads/main\n");
            assert_eq!(text(metadata.join("config")), "[core]\n\tbare = false\n");
            if !is_directory {
                assert_eq!(text(&git), pointer);
            }
            assert!(sandbox.work.join(format!("{pm}.toml")).is_file());
            assert!(
                text(sandbox.work.join(".rogrid/generated/shared/Server.luau"))
                    .contains("setReady")
            );
        }
    }
}

#[test]
fn current_directory_and_force_preserve_unrelated_files() {
    let sandbox = Sandbox::new();
    sandbox.provide("rokit");
    fs::create_dir(sandbox.work.join(".git")).unwrap();
    fs::write(sandbox.work.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::write(sandbox.work.join("keep.txt"), "keep me").unwrap();
    failure(
        sandbox
            .command("wally", "rokit")
            .args([".", "--name", "my-game"])
            .output()
            .unwrap(),
        "is not empty",
    );
    assert!(sandbox.commands().is_empty());
    assert!(!sandbox.work.join("wally.toml").exists());
    success(
        sandbox
            .command("wally", "rokit")
            .args([".", "--name", "my-game", "--force"])
            .output()
            .unwrap(),
    );
    assert_eq!(text(sandbox.work.join("keep.txt")), "keep me");
    assert_eq!(
        text(sandbox.work.join(".git/HEAD")),
        "ref: refs/heads/main\n"
    );
    assert!(sandbox.work.join("wally.toml").is_file());
}

#[test]
fn invalid_names_and_local_source_fail_before_writes() {
    for (pm, name) in [
        ("wally", "a".repeat(65)),
        ("pesde", "a".repeat(33)),
        ("pesde", "123".into()),
    ] {
        let sandbox = Sandbox::new();
        failure(
            sandbox
                .command(pm, "rokit")
                .args(["game", "--name", &name])
                .output()
                .unwrap(),
            "package names",
        );
        assert!(!sandbox.work.join("game").exists());
        assert!(sandbox.commands().is_empty());
    }
    let sandbox = Sandbox::new();
    failure(
        sandbox
            .command("wally", "rokit")
            .arg("game")
            .arg("--local-framework")
            .arg(sandbox.work.join("missing"))
            .output()
            .unwrap(),
        "must contain src/init.luau",
    );
    assert!(!sandbox.work.join("game").exists());
}

#[test]
fn noninteractive_missing_choices_report_how_to_supply_them() {
    let sandbox = Sandbox::new();
    failure(
        sandbox.command("wally", "rokit").output().unwrap(),
        "cannot ask for the project name",
    );
    assert!(sandbox.commands().is_empty());
}
