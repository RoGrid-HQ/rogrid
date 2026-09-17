//! Generates the code that makes requests callable: input checkers for the
//! server, and a typed `api` module for the client.

mod emit;
mod parse;
mod types;
mod validator;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::project::Project;
use emit::Entry;

/// Every request lives here. The path below it is the request's name.
pub const FUNCTIONS_DIR: &str = "src/server/functions";
pub const CLIENT_OUTPUT: &str = ".rogrid/client/api.luau";
pub const SERVER_OUTPUT: &str = ".rogrid/server/functions.luau";

/// What one run did, for the commands to report.
pub struct Summary {
    pub count: usize,
    /// Whether any generated file is different from what was on disk.
    pub changed: bool,
    /// Whether the Rojo project maps `.rogrid` into the game.
    pub mapped: bool,
}

/// Generates from the project in `root` and writes `.rogrid`.
/// When a request has a problem nothing is written, so the last good output stays in place.
pub fn run(root: &Path) -> Result<Summary> {
    let project = Project::load(root)?;
    let generated = generate(&project)?;

    let client = write(&root.join(CLIENT_OUTPUT), &generated.client)?;
    let server = write(&root.join(SERVER_OUTPUT), &generated.server)?;

    Ok(Summary {
        count: generated.count,
        changed: client || server,
        mapped: [CLIENT_OUTPUT, SERVER_OUTPUT]
            .iter()
            .all(|output| project.game_path(Path::new(output)).is_some()),
    })
}

struct Generated {
    client: String,
    server: String,
    count: usize,
}

fn generate(project: &Project) -> Result<Generated> {
    let mut entries = Vec::new();
    let mut problems = Vec::new();

    for (name, file) in discover(&project.root.join(FUNCTIONS_DIR))? {
        let shown = file
            .strip_prefix(&project.root)
            .unwrap_or(&file)
            .display()
            .to_string();

        if let Some(segment) = name.split('/').find(|segment| !is_identifier(segment)) {
            problems.push(format!(
                "error: {shown}\n  `{segment}` cannot be used as a name in Luau. Rename it"
            ));
            continue;
        }

        let source =
            fs::read_to_string(&file).with_context(|| format!("could not read {shown}"))?;
        match parse::parse(&name, &source) {
            Ok(function) => {
                let require_path = project.game_path(&file).with_context(|| {
                    format!("{shown} is not mapped into the game by default.project.json")
                })?;
                entries.push(Entry {
                    function,
                    require_path,
                });
            }
            Err(problem) => {
                problems.push(format!(
                    "error: {shown}:{}\n  {}",
                    problem.line, problem.message
                ));
            }
        }
    }

    // `shop/buy` with a type `ItemInput` and `shop/buyItem` with a type `Input` would both
    // produce `ShopBuyItemInput`.
    let mut type_names = HashSet::new();
    for (name, _) in entries.iter().flat_map(|entry| &entry.function.aliases) {
        if !type_names.insert(name) {
            problems.push(format!(
                "error: two requests both produce the generated type `{name}`\n  Rename one of the types involved"
            ));
        }
    }

    if !problems.is_empty() {
        bail!(
            "{} problem(s) in your requests\n\n{}",
            problems.len(),
            problems.join("\n\n")
        );
    }

    let library = if entries.is_empty() {
        String::new()
    } else {
        project.library().context(
            "could not find the rogrid library in your Rojo project. Install your packages first",
        )?
    };

    Ok(Generated {
        client: emit::client(&entries, &library).map_err(anyhow::Error::msg)?,
        server: emit::server(&entries),
        count: entries.len(),
    })
}

/// Writes a file unless it already holds `contents`, and says whether it wrote.
/// Leaving an unchanged file alone means Rojo has nothing to sync.
fn write(path: &Path, contents: &str) -> Result<bool> {
    if fs::read_to_string(path).is_ok_and(|existing| existing == contents) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents).with_context(|| format!("could not write {}", path.display()))?;
    Ok(true)
}

/// Every request file below `dir` as (name, path), sorted so the output is stable.
/// Anything starting with `_` or `.` is skipped, which leaves room for files like `_guard.luau`.
fn discover(dir: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut found = Vec::new();
    if dir.is_dir() {
        walk(dir, "", &mut found)?;
    }
    found.sort();
    Ok(found)
}

fn walk(dir: &Path, prefix: &str, found: &mut Vec<(String, PathBuf)>) -> Result<()> {
    let entries = fs::read_dir(dir).with_context(|| format!("could not read {}", dir.display()))?;

    for entry in entries {
        let path = entry?.path();
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if stem.starts_with('_') || stem.starts_with('.') {
            continue;
        }

        let name = format!("{prefix}{stem}");
        if path.is_dir() {
            walk(&path, &format!("{name}/"), found)?;
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("luau" | "lua")
        ) {
            found.push((name, path));
        }
    }

    Ok(())
}

fn is_identifier(text: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "if", "in",
        "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
    ];

    let mut chars = text.chars();
    let starts_well = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    starts_well && chars.all(|c| c.is_ascii_alphanumeric() || c == '_') && !KEYWORDS.contains(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROJECT: &str = r#"{
        "name": "test",
        "tree": {
            "$className": "DataModel",
            "ReplicatedStorage": { "Packages": { "$path": { "optional": "roblox_packages" } } },
            "ServerScriptService": { "Server": { "$path": "src/server" } }
        }
    }"#;

    fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, Project) {
        let dir = tempfile::tempdir().unwrap();
        for (path, contents) in files.iter().chain(&[("default.project.json", PROJECT)]) {
            let path = dir.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
        let project = Project::load(dir.path()).unwrap();
        (dir, project)
    }

    #[test]
    fn generates_both_sides_from_a_request_file() {
        let (_dir, project) = project(&[
            ("roblox_packages/rogrid.luau", "return {}"),
            (
                "src/server/functions/shop/buyItem.luau",
                "type Input = { itemId: string }\nreturn RoGrid.request(function(player: Player, input: Input): number return 1 end)",
            ),
            (
                "src/server/functions/shop/_guard.luau",
                "return function() return true end",
            ),
        ]);

        let generated = generate(&project).unwrap();

        assert_eq!(generated.count, 1);
        assert!(generated.server.contains(
            r#"definition = require("@game/ServerScriptService/Server/functions/shop/buyItem"),"#
        ));
        assert!(
            generated
                .server
                .contains("check = checks.ShopBuyItemInput,")
        );
        assert!(
            generated
                .client
                .contains(r#"require("@game/ReplicatedStorage/Packages/rogrid")"#)
        );
        assert!(
            generated
                .client
                .contains("export type ShopBuyItemInput = { itemId: string }")
        );
        assert!(generated.client.contains(
            r#"buyItem = (caller("shop/buyItem") :: any) :: (input: ShopBuyItemInput) -> number,"#
        ));
    }

    #[test]
    fn run_writes_the_output_and_only_reports_a_change_once() {
        let (dir, _project) = project(&[
            ("roblox_packages/rogrid.luau", "return {}"),
            (
                "src/server/functions/ping.luau",
                "return RoGrid.request(function(player: Player): string return \"pong\" end)",
            ),
        ]);

        let first = run(dir.path()).unwrap();
        assert!(first.changed);
        assert_eq!(first.count, 1);
        // The test project does not map `.rogrid`.
        assert!(!first.mapped);
        assert!(dir.path().join(CLIENT_OUTPUT).exists());
        assert!(dir.path().join(SERVER_OUTPUT).exists());

        assert!(!run(dir.path()).unwrap().changed);
    }

    #[test]
    fn a_problem_leaves_the_last_good_output_in_place() {
        let (dir, _project) = project(&[
            ("roblox_packages/rogrid.luau", "return {}"),
            (
                "src/server/functions/ping.luau",
                "return RoGrid.request(function(player: Player): string return \"pong\" end)",
            ),
        ]);
        run(dir.path()).unwrap();
        let before = fs::read_to_string(dir.path().join(CLIENT_OUTPUT)).unwrap();

        fs::write(
            dir.path().join("src/server/functions/ping.luau"),
            "return 1",
        )
        .unwrap();

        assert!(run(dir.path()).is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join(CLIENT_OUTPUT)).unwrap(),
            before
        );
    }

    #[test]
    fn a_project_without_requests_still_generates() {
        let (_dir, project) = project(&[]);
        let generated = generate(&project).unwrap();
        assert_eq!(generated.count, 0);
        assert!(generated.client.ends_with("return {}\n"));
    }

    #[test]
    fn every_problem_is_reported_with_its_file_and_line() {
        let (_dir, project) = project(&[
            (
                "src/server/functions/a.luau",
                "return RoGrid.request(function(player: Player, input: any) end)",
            ),
            ("src/server/functions/b.luau", "return 1"),
            ("src/server/functions/bad-name.luau", "return 1"),
        ]);

        let report = generate(&project).err().unwrap().to_string();

        assert!(report.starts_with("3 problem(s)"));
        assert!(report.contains("a.luau:1"));
        assert!(report.contains("`bad-name` cannot be used as a name"));
    }

    #[test]
    fn output_types_get_no_checker() {
        let (_dir, project) = project(&[
            ("roblox_packages/rogrid.luau", "return {}"),
            (
                "src/server/functions/getItem.luau",
                "type Item = { id: string }\nreturn RoGrid.request(function(player: Player): Item return { id = \"a\" } end)",
            ),
        ]);

        let generated = generate(&project).unwrap();

        assert!(
            generated
                .client
                .contains("export type GetItemItem = { id: string }")
        );
        assert!(!generated.server.contains("checks.GetItemItem"));
    }

    #[test]
    fn generated_type_names_must_be_unique() {
        let (_dir, project) = project(&[
            ("roblox_packages/rogrid.luau", "return {}"),
            (
                "src/server/functions/shop/buy.luau",
                "type ItemInput = { id: string }\nreturn RoGrid.request(function(player: Player, input: ItemInput) end)",
            ),
            (
                "src/server/functions/shop/buyItem.luau",
                "type Input = { id: string }\nreturn RoGrid.request(function(player: Player, input: Input) end)",
            ),
        ]);
        let report = generate(&project).err().unwrap().to_string();
        assert!(report.contains("`ShopBuyItemInput`"));
    }

    #[test]
    fn a_name_cannot_be_both_a_request_and_a_folder() {
        let request = "return RoGrid.request(function(player: Player) end)";
        let (_dir, project) = project(&[
            ("roblox_packages/rogrid.luau", "return {}"),
            ("src/server/functions/shop.luau", request),
            ("src/server/functions/shop/buyItem.luau", request),
        ]);
        assert!(
            generate(&project)
                .err()
                .unwrap()
                .to_string()
                .contains("clashes")
        );
    }
}
