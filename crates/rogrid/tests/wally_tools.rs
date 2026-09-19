//! Opt-in checks against real external tools, separate from the isolated CLI suite.
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
#[ignore = "requires Wally 0.3.2 and Rojo 7.7 on PATH; may update Wally's registry cache"]
fn local_wally_project_generates_and_builds_with_real_tools() {
    let dir = tempfile::tempdir().unwrap();
    let package = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/rogrid");
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
            .arg(package)
            .output()
            .unwrap(),
    );
    let project = dir.path().join("game");
    assert!(project.join("wally.lock").is_file());
    assert!(!project.join("pesde.toml").exists());
    let revision =
        fs::read_to_string(project.join(".rogrid/generated/shared/Revision.luau")).unwrap();
    assert!(!revision.contains("invalid"));
    success(
        Command::new(env!("CARGO_BIN_EXE_rogrid"))
            .current_dir(&project)
            .args(["dev", "--once"])
            .output()
            .unwrap(),
    );
    let model = dir.path().join("game.rbxl");
    success(
        Command::new("rojo")
            .current_dir(&project)
            .args(["build", "default.project.json", "--output"])
            .arg(&model)
            .output()
            .unwrap(),
    );
    assert!(fs::metadata(model).unwrap().len() > 0);

    // Wally itself must accept the released dependency's exact version syntax.
    let starter = fs::read_to_string(project.join("wally.toml")).unwrap();
    fs::write(
        project.join("wally.toml"),
        starter.replace(
            "# RoGrid is mapped from local source in default.project.json.",
            "rogrid = \"rogrid-hq/rogrid@=0.2.1\"",
        ),
    )
    .unwrap();
    let manifest = success(
        Command::new("wally")
            .current_dir(&project)
            .arg("manifest-to-json")
            .output()
            .unwrap(),
    );
    let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    assert_eq!(
        manifest["dependencies"]["rogrid"],
        "rogrid-hq/rogrid@=0.2.1"
    );
}

#[test]
#[ignore = "requires Wally 0.3.2 and Rojo 7.7 on PATH"]
fn library_packages_with_a_module_entrypoint_and_runtime_child() {
    let dir = tempfile::tempdir().unwrap();
    let package = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/rogrid");
    let archive = dir.path().join("rogrid.zip");
    let files = success(
        Command::new("wally")
            .current_dir(&package)
            .args(["package", "--list", "--output"])
            .arg(&archive)
            .output()
            .unwrap(),
    );
    for file in [
        "wally.toml",
        "default.project.json",
        "src/init.luau",
        "src/runtime.luau",
        "LICENSE",
    ] {
        assert!(
            files
                .replace('\\', "/")
                .lines()
                .any(|line| line.strip_prefix("./").unwrap_or(line) == file),
            "missing {file} in {files}"
        );
    }
    assert!(!files.contains("pesde.toml"));
    assert!(!files.contains("pesde.lock"));
    success(
        Command::new("wally")
            .current_dir(&package)
            .args(["package", "--output"])
            .arg(&archive)
            .output()
            .unwrap(),
    );
    assert!(fs::metadata(archive).unwrap().len() > 0);
    let map = success(
        Command::new("rojo")
            .current_dir(&package)
            .args(["sourcemap", "default.project.json"])
            .output()
            .unwrap(),
    );
    let map: serde_json::Value = serde_json::from_str(&map).unwrap();
    assert_eq!(map["name"], "rogrid");
    assert_eq!(map["className"], "ModuleScript");
    assert!(
        map["children"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["name"] == "runtime" && node["className"] == "ModuleScript")
    );
}
