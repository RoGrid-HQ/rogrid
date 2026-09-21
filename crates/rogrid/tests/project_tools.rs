//! Run in the package CI job, with real installers and the Roblox typechecker.
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
#[ignore = "requires Pesde, Wally, Rojo, luau-lsp and ROGRID_ROBLOX_DEFINITIONS; installs project dependencies"]
fn real_manager_projects_build_and_complete_generated_modules_typecheck() {
    let definitions =
        std::env::var_os("ROGRID_ROBLOX_DEFINITIONS").expect("set ROGRID_ROBLOX_DEFINITIONS");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    for manager in ["wally", "pesde"] {
        success(
            Command::new(env!("CARGO_BIN_EXE_rogrid"))
                .current_dir(dir.path())
                .stdin(Stdio::null())
                .args([
                    "init",
                    manager,
                    "--package-manager",
                    manager,
                    "--tool-manager",
                    "none",
                    "--local-framework",
                ])
                .arg(repo.join("packages/rogrid"))
                .output()
                .unwrap(),
        );
        let project = dir.path().join(manager);
        for (source, target) in [
            ("Events.server.luau", "src/server/events/Lobby.luau"),
            ("Events.client.luau", "src/client/events/Notifications.luau"),
        ] {
            fs::copy(
                repo.join("packages/rogrid/tests/project").join(source),
                project.join(target),
            )
            .unwrap();
        }
        // The stock demo calls setReady; these fixtures exercise different events.
        fs::write(
            project.join("src/client/ReadyDemo.luau"),
            "--!strict\nreturn function() end\n",
        )
        .unwrap();
        success(
            Command::new(env!("CARGO_BIN_EXE_rogrid"))
                .current_dir(&project)
                .args(["dev", "--once"])
                .output()
                .unwrap(),
        );
        success(
            Command::new("rojo")
                .current_dir(&project)
                .args(["build", "--output"])
                .arg(dir.path().join(format!("{manager}.rbxl")))
                .output()
                .unwrap(),
        );
        success(
            Command::new("rojo")
                .current_dir(&project)
                .args([
                    "sourcemap",
                    "default.project.json",
                    "--output",
                    "sourcemap.json",
                ])
                .output()
                .unwrap(),
        );
        success(
            Command::new("luau-lsp")
                .current_dir(&project)
                .args([
                    "analyze",
                    "--platform=roblox",
                    "--sourcemap=sourcemap.json",
                    "--definitions",
                ])
                .arg(&definitions)
                .args(["src", ".rogrid/generated"])
                .output()
                .unwrap(),
        );
        let incorrect = project.join("src/client/Incorrect.luau");
        fs::write(
            &incorrect,
            r#"--!strict
local Server = require(game:GetService("ReplicatedStorage").RoGridGenerated.Server)
local wrongResult: string = Server.Lobby.inspect.invoke({id = "hello"})
Server.Lobby.inspect.invoke("wrong argument")
Server.Lobby.acknowledge.invoke("wrong timeout")
return wrongResult
"#,
        )
        .unwrap();
        let rejected = Command::new("luau-lsp")
            .current_dir(&project)
            .args([
                "analyze",
                "--platform=roblox",
                "--sourcemap=sourcemap.json",
                "--definitions",
            ])
            .arg(&definitions)
            .arg("src/client/Incorrect.luau")
            .output()
            .unwrap();
        let diagnostics = format!(
            "{}{}",
            String::from_utf8_lossy(&rejected.stdout),
            String::from_utf8_lossy(&rejected.stderr)
        );
        assert!(
            !rejected.status.success() && diagnostics.matches("TypeError").count() >= 3,
            "{diagnostics}"
        );
        fs::remove_file(incorrect).unwrap();
    }
}
