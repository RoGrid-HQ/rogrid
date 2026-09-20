//! Real engine integration: opt in on a machine with Studio and run-in-roblox.
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
#[ignore = "requires Roblox Studio, Wally, Rojo and ROGRID_STUDIO_RUNNER pointing to run-in-roblox"]
fn initialized_project_starts_and_routes_generated_events_between_two_clients() {
    let runner = std::env::var_os("ROGRID_STUDIO_RUNNER").expect("set ROGRID_STUDIO_RUNNER");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    success(
        Command::new(env!("CARGO_BIN_EXE_rogrid"))
            .current_dir(dir.path())
            .stdin(Stdio::null())
            .args([
                "init",
                "game",
                "--package-manager",
                "wally",
                "--tool-manager",
                "none",
                "--local-framework",
            ])
            .arg(repo.join("packages/rogrid"))
            .output()
            .unwrap(),
    );
    let project = dir.path().join("game");
    for (source, target) in [
        ("Events.server.luau", "src/server/events/Lobby.luau"),
        ("Events.client.luau", "src/client/events/Notifications.luau"),
        ("main.server.luau", "src/server/main.server.luau"),
        ("main.client.luau", "src/client/main.client.luau"),
    ] {
        fs::copy(
            repo.join("packages/rogrid/tests/project").join(source),
            project.join(target),
        )
        .unwrap();
    }
    success(
        Command::new(env!("CARGO_BIN_EXE_rogrid"))
            .current_dir(&project)
            .args(["dev", "--once"])
            .output()
            .unwrap(),
    );
    for linker in [false, true] {
        if linker {
            // Exercise the same public import through a package-manager-style linker.
            fs::write(
                project.join("linker.luau"),
                "return require(script.Parent.Bundled)\n",
            )
            .unwrap();
            let path = project.join("default.project.json");
            let mut map: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            let packages = map.pointer_mut("/tree/ReplicatedStorage/Packages").unwrap();
            packages["Bundled"] = packages["rogrid"].clone();
            packages["rogrid"] = serde_json::json!({"$path": "linker.luau"});
            fs::write(path, serde_json::to_string_pretty(&map).unwrap()).unwrap();
        }
        let model = dir.path().join(if linker {
            "linked.rbxlx"
        } else {
            "direct.rbxlx"
        });
        success(
            Command::new("rojo")
                .current_dir(&project)
                .args(["build", "--output"])
                .arg(&model)
                .output()
                .unwrap(),
        );
        let output = success(
            Command::new(&runner)
                .args(["--place"])
                .arg(model)
                .arg("--script")
                .arg(repo.join("packages/rogrid/tests/studio-multiplayer.luau"))
                .output()
                .unwrap(),
        );
        assert!(output.contains("PASS: generated startup"), "{output}");
    }
}
