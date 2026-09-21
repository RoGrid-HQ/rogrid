---
sidebar_position: 6
---

# CLI reference

RoGrid has two commands: `init` creates a project, and `dev` generates network
interfaces and runs the development server. Use `rogrid --help` or a command's
`--help` flag to inspect the installed binary.

See [package managers](./package-managers.md) for supported Pesde and Wally
setups and [project status](./status.md) for current limitations.

## rogrid init

```sh
rogrid init [FOLDER] [OPTIONS]
```

Creates a starter project, writes manager manifests, installs tools and
packages, and generates event and request code. Missing choices are prompted for.

| Argument or option | Description |
| --- | --- |
| `[FOLDER]` | New folder under the current directory, or `.` to use the current folder. |
| `--name <NAME>` | Project name. Defaults to the new folder name; prompted for when using `.`. |
| `--force` | Allow a nonempty folder and overwrite files written by the starter. |
| `--package-manager <NAME>` | `pesde` or `wally`. |
| `--tool-manager <NAME>` | `rokit`, `pesde`, or `none`, subject to manager compatibility. |
| `--local-framework <PATH>` | Use a local RoGrid package instead of a registry dependency. |
| `-h`, `--help` | Show command help. |

Provide all choices for a non-interactive setup:

```sh
rogrid init my-game --package-manager pesde --tool-manager rokit
rogrid init . --name my-game --package-manager pesde --tool-manager rokit
```

When only `--name` is supplied, it also supplies the new folder name. To
create a project elsewhere, change to its parent directory first. `FOLDER`
accepts a simple folder name or `.`, not an arbitrary path.

The target must be empty apart from an existing `.git` file or folder,
which is preserved. This lets you initialize a project after `git init` or
inside an empty Git worktree. Other existing files or folders require
`--force`, which overwrites matching starter files rather than merging
RoGrid into an existing game.

The CLI rejects incompatible managers, invalid package names,
and missing externally supplied tools before writing files. See
[package managers](./package-managers.md) for supported combinations.

### Local framework

`--local-framework` points to the RoGrid package folder containing
`src/init.luau`. From a checkout's root:

```sh
cargo run -p rogrid -- init my-game --package-manager pesde --tool-manager rokit --local-framework packages/rogrid
cd my-game
cargo run --manifest-path ../Cargo.toml -p rogrid -- dev
```

For a game outside the checkout, run Cargo from the game's parent folder
with `--manifest-path /path/to/rogrid/Cargo.toml` and use
`--local-framework /path/to/rogrid/packages/rogrid`. Replace these paths with
your checkout's location and quote paths containing spaces.

The framework path is resolved relative to the directory where `init` runs,
or can be absolute. RoGrid omits the published framework dependency and the
CLI tool pin, then maps local source to `ReplicatedStorage.Packages.rogrid`.
Other tools and dependencies still install normally.

The generated Rojo project stores an absolute source path. Keep the checkout
in that location and continue using its CLI for `dev`. Restart Studio Play
after Luau changes; restart Cargo after Rust changes.

## rogrid dev

```sh
rogrid dev [--once]
```

Run this inside a game folder containing `default.project.json`. Rojo must
be available on PATH, including for `--once`.

| Option | Description |
| --- | --- |
| `--once` | Generate once without watching files or starting the Rojo server. |
| `-h`, `--help` | Show command help. |

Without `--once`, the command generates interfaces, watches project Luau
sources (including shared type aliases) and project settings, and starts
`rojo serve default.project.json`.
Ctrl+C stops both the watcher and the Rojo process.

Generation errors at startup exit the command. Errors while watching are
reported and block generated startup until valid source is saved. Restart
Play after successful generation to load the changes.

For a build:

```sh
rogrid dev --once
rojo build -o game.rbxl
```

See [configuration](./configuration.md) for watched files, generated output,
and editor settings.
