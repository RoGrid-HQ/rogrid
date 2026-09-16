# RoGrid

A framework for building Roblox games in Luau, with a CLI that sets up a
project the way `create-next-app` does for Next.js: one command, a few
questions, and you are in Studio.

> **Status:** early development. The CLI scaffolds and installs, but the
> `rogrid` library it installs is a placeholder while the framework is being
> built. Expect breaking changes until 1.0.

## Getting started

Install the CLI with [Rokit](https://github.com/rojo-rbx/rokit):

```sh
rokit add RoGrid-HQ/rogrid
```

Create a project:

```sh
rogrid init my-game
```

You will be asked which package manager and tool manager to use. Then RoGrid
creates the folder, writes the manifests, installs the tools and the library,
and tells you what to run next:

```sh
cd my-game
rojo serve
```

### Non-interactive

Every prompt has a flag, so the same command works in scripts and CI:

```sh
rogrid init my-game --package-manager pesde --tool-manager rokit
```

Run `rogrid init --help` for the accepted values.

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

| Role            | Available today | Coming            |
| --------------- | --------------- | ----------------- |
| Package manager | pesde           | Wally, Ember      |
| Tool manager    | Rokit, pesde, none | Aftman, Foreman, mise |

A tool manager installs binaries such as Rojo. A package manager installs Luau
libraries such as `rogrid`. pesde can do both, in which case Rojo is added as a
dev dependency and no `rokit.toml` is written.

## Repository layout

```
crates/rogrid/      the CLI (Rust)
packages/rogrid/    the core library (Luau), published to pesde as rogrid/rogrid
templates/          project templates embedded into the CLI at build time
```

## Development

Requires a stable Rust toolchain.

```sh
cargo run -- init my-game
```

To try the CLI from another folder without installing it:

```sh
cargo run --manifest-path path/to/rogrid/Cargo.toml -- init my-game
```

`cargo fmt`, `cargo clippy` and a release build run on every push for Linux,
macOS and Windows.

## License

MIT
