# RoGrid

[![CI](https://github.com/RoGrid-HQ/rogrid/actions/workflows/ci.yml/badge.svg)](https://github.com/RoGrid-HQ/rogrid/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/RoGrid-HQ/rogrid)](https://github.com/RoGrid-HQ/rogrid/releases/latest)

A framework for building Roblox games in Luau, with a CLI that sets up a
project the way `create-next-app` does for Next.js: one command, a few
questions, and you are in Studio.

> **Status:** early development. The CLI scaffolds and installs, but the
> `rogrid` library it installs is a placeholder while the framework is being
> built. Expect breaking changes until 1.0.

## Installation

You need [Roblox Studio](https://create.roblox.com/) and
[Rokit](https://github.com/rojo-rbx/rokit#installation). Then:

```sh
rokit add --global RoGrid-HQ/rogrid
```

`--global` makes `rogrid` available in every folder. It needs that because it
is the tool that creates project folders.

## Usage

```sh
rogrid init my-game
```

RoGrid asks which package manager and tool manager to use, then creates the
folder, writes the manifests, and installs the tools and the library. Open the
project in Studio:

```sh
cd my-game
rojo plugin install
rojo serve
```

The plugin only needs installing once. Open a place in Studio, click Connect
in the Rojo plugin, and the code in `src/` is in the game.

### In the current folder

```sh
rogrid init .
```

RoGrid asks for a project name and suggests one made from the folder's name.
The folder has to be empty. A freshly cloned repository is not, because of its
`.git` folder, so pass `--force` there. `--force` lets RoGrid write into a
folder that already has files in it. Any file it writes replaces the existing
one, so do not use it to add RoGrid to an existing project.

### Non-interactive

Every prompt has a flag, so the same command works in scripts and CI:

```sh
rogrid init my-game --package-manager pesde --tool-manager rokit
rogrid init . --name my-game --package-manager pesde --tool-manager rokit
```

### Options

| Option                     | Description                                                                |
| -------------------------- | -------------------------------------------------------------------------- |
| `[FOLDER]`                 | Folder to create, or `.` for the current folder. Prompted for when omitted. |
| `--name <NAME>`            | Project name. Defaults to the folder name; prompted for with `.`.          |
| `--force`                  | Set up in a folder that is not empty. Files RoGrid writes are replaced.    |
| `--package-manager <NAME>` | `pesde`                                                                    |
| `--tool-manager <NAME>`    | `rokit`, `pesde` or `none`                                                 |

`rogrid init --help` is always up to date.

## What you get

```
my-game/
  default.project.json    Rojo project mapping the folders below into the game
  pesde.toml              dependencies, with the rogrid library already listed
  rokit.toml              pinned tool versions
  src/
    client/               StarterPlayerScripts
    server/               ServerScriptService
    shared/               ReplicatedStorage
```

Installed packages land in the package manager's folder and are ignored by git.

## Supported tools

RoGrid does not manage packages or binaries itself. It writes the manifests for
the tools you already use and runs their install commands.

| Role            | Available today    | Coming                |
| --------------- | ------------------ | --------------------- |
| Package manager | pesde              | Wally, Ember          |
| Tool manager    | Rokit, pesde, none | Aftman, Foreman, mise |

A tool manager installs binaries such as Rojo. A package manager installs Luau
libraries such as `rogrid`. pesde can do both, in which case Rojo is added as a
dev dependency and no `rokit.toml` is written.

## Development

The repository has three parts:

```
crates/rogrid/      the CLI (Rust)
packages/rogrid/    the core library (Luau), published to pesde as rogrid/rogrid
templates/          project templates embedded into the CLI at build time
```

Building the CLI needs a stable Rust toolchain:

```sh
cargo run -- init my-game
```

To try it from another folder without installing it:

```sh
cargo run --manifest-path path/to/rogrid/Cargo.toml -- init my-game
```

`cargo fmt`, `cargo clippy` and a release build run on every push for Linux,
macOS and Windows.

## License

MIT
