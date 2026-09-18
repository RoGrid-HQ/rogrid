# RoGrid

[![CI](https://github.com/RoGrid-HQ/rogrid/actions/workflows/ci.yml/badge.svg)](https://github.com/RoGrid-HQ/rogrid/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/RoGrid-HQ/rogrid)](https://github.com/RoGrid-HQ/rogrid/releases/latest)

A framework for building Roblox games in Luau, with a CLI that sets up a
project the way `create-next-app` does for Next.js: one command, a few
questions, and you are in Studio.

> **Status:** early development. CLI `0.3.0` and Luau package `0.2.0` provide
> generated, typed events in both directions. Expect breaking changes until 1.0.
> Use the [local playground](playground/README.md) to develop without publishing.
> The starter template requires Luau package `0.2.0` in the registry; publish
> the package before releasing CLI `0.3.0`.

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
folder, writes the manifests, and installs the tools and dependencies. Open the
project in Studio:

```sh
cd my-game
rojo plugin install
rogrid dev
```

The plugin only needs installing once. Open a place in Studio, click Connect
in the Rojo plugin, then press Play. The starter sets the player's ready status
on the server and sends a notification to the clients, visible in Output.

`rogrid dev` generates the event interfaces, watches configured event folders
and project settings, and starts Rojo.
Restart Play after edits; this version does not hot-reload running modules.
Install the recommended Luau Language Server extension in VS Code for typed
callers and argument diagnostics. The template includes its settings.

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
| `--local-framework <PATH>` | Use a local RoGrid package folder instead of downloading RoGrid.         |

`rogrid init --help` is always up to date.

## What you get

```
my-game/
  default.project.json    Rojo project mapping the folders below into the game
  pesde.toml              your game's package dependencies
  rokit.toml              pinned tool versions
  .rogrid/generated/     typed callers, validation, and startup wiring
  src/
    client/events/        client receivers; generated server callers target them
    server/events/        server receivers; generated client callers target them
    shared/               ReplicatedStorage
```

Installed packages land in the package manager's folder and are ignored by git.

## Typed events

Write a receiver once, on the side that handles it:

```luau
-- src/server/events/Lobby.luau
--!strict
local RoGrid = require(game.ReplicatedStorage.Packages.rogrid)

return {
    setReady = RoGrid.event(function(player: Player, ready: boolean)
        player:SetAttribute("Ready", ready)
    end),
}
```

Call it from the client after the template's `RoGrid.start()` call:

```luau
local Server = require(game.ReplicatedStorage.RoGridGenerated.Server)
Server.Lobby.setReady.fire(true)
```

The first server argument is always the real sending player; clients do not
provide it. For the other direction, declare a receiver in `src/client/events/`
and use the generated server-only interface:

```luau
local Client = require(game.ServerScriptService.RoGridGenerated.Client)
Client.Notifications.show.fire(player, "Round starting!")
Client.Notifications.show.fireAll("A new round is starting!")
```

There is one ordinary RemoteEvent per declaration. No requests, replies,
promises, unreliable events, or handwritten shared definitions.

Payload arguments must be explicitly typed `string`, `boolean`, or `number`.
Generation rejects unsupported types and declarations. Incoming payloads are
checked for exact argument count and types; numbers must be finite and strings
are limited to 4096 bytes. The server allows a burst of 60 inbound events per
player, refilling at 60 per second across all events. Invalid or excessive
messages are dropped. Game-specific authorization and cooldowns still belong
in your handlers. [Full event API and limitations](packages/rogrid/docs/events.md).

The runtime is installed as the `rogrid` package and loaded from
`ReplicatedStorage.Packages.rogrid`. The CLI generates only game-specific code;
it does not copy the framework. Generated code checks the runtime protocol
before starting. Do not edit `.rogrid/generated/`; it is rebuilt from source
and can be deleted. Commit the receiver files and project settings.

For a build without the watcher:

```sh
rogrid dev --once
rojo build -o game.rbxl
```

Run generation before every build. Missing, failed, or inconsistent generation
blocks startup; source signatures cannot be inspected by the running game.

Generation prints the total event count, followed by `+` for added events, `-`
for removed events, and `~` for changed argument names or types. Handler-body
edits do not appear in this list. The previous successful event inventory is
cached in `.rogrid/events.json`; without it, all current events appear as added.

On existing projects, `rogrid dev` adds the three `RoGridGenerated` Rojo mappings
but preserves existing VS Code settings. If necessary, enable
`luau-lsp.diagnostics.strictDatamodelTypes` and Rojo sourcemaps yourself. Event
receivers go directly inside the configured folders, and the startup scripts
call `RoGrid.start()` once per side. The framework handles receiver paths and
startup checks. The starter's optional ready-status example is in
`src/client/ReadyDemo.luau`, called after startup.

### Choosing event folders

No configuration is needed for `src/server/events` and `src/client/events`.
To override them, add `rogrid.toml` in the game folder:

```toml
[events]
server = ["src/server/events", "src/server/cooler_events"]
client = ["src/client/events"]
```

Each list replaces that side's default. Omitted sides keep their defaults;
`server = []` or `client = []` disables discovery for that side. Folders must
exist, use project-relative paths, and stay inside the project. Only direct
`.luau`/`.lua` modules are discovered; list subfolders separately if needed.
Module names must be unique per side, so two server folders cannot both
contain `Lobby.luau`. Caller names remain `Server.Lobby.setReady.fire(...)`.

Map the folders into the game in `default.project.json`. RoGrid uses a fresh
Rojo sourcemap to locate each module; it does not assume disk paths match
Roblox paths. Server receivers must map under `ServerScriptService` or
`ServerStorage`; client receivers under `StarterPlayerScripts` or
`ReplicatedStorage`. Each receiver must map to exactly one ModuleScript.
`rogrid dev` reloads configuration changes automatically. Missing folders,
invalid configuration, and ambiguous mappings block generation with an error.
Rojo must be installed even for `rogrid dev --once`.

## Supported tools

RoGrid does not manage packages or binaries itself. It writes the manifests for
the tools you already use and runs their install commands.

| Role            | Available today    | Coming                |
| --------------- | ------------------ | --------------------- |
| Package manager | pesde              | Wally, Ember          |
| Tool manager    | Rokit, pesde, none | Aftman, Foreman, mise |

A tool manager installs binaries such as Rojo. A package manager installs Luau
libraries such as `rogrid`. Every Pesde project also declares Rojo as a dev
dependency, as required by Pesde's Rojo configuration helper. In Rokit mode,
both declarations start with the same Rojo version; keep them aligned when
updating tools. Using both launchers can download separate copies of Rojo.

Choosing `pesde` or `none` writes no `rokit.toml`. `none` skips separate
tool-manager setup, but still installs package dependencies, including Pesde's
Rojo helper and launcher.

## Development

The repository contains:

```
crates/rogrid/      the CLI (Rust)
packages/rogrid/    the core library (Luau), installed as a package
templates/         project templates embedded into the CLI at build time
playground/        a small game using the local CLI and framework source
```

For everyday development, use the [playground](playground/README.md):

```sh
cd playground
rokit install
cargo run --manifest-path ../Cargo.toml -p rogrid -- dev
```

It uses the framework source directly through Rojo. No package publication or
installation is needed. Restart Play after Luau edits; restart the command
after Rust edits.

### Create a separate game with the local framework

Run your checkout's CLI from the folder where you want the new game:

```sh
cargo run --manifest-path /path/to/RoGrid/Cargo.toml -p rogrid -- \
  init my-game --local-framework /path/to/RoGrid/packages/rogrid
cd my-game
cargo run --manifest-path /path/to/RoGrid/Cargo.toml -p rogrid -- dev
```

Replace `/path/to/RoGrid` with your checkout's absolute path. Quote paths that
contain spaces. `init` asks for package and tool managers as usual; you can
also pass `--package-manager pesde --tool-manager rokit`.

The local framework path can be absolute or relative to the directory where
you run `init`, and must point to the package folder containing `src/init.luau`.
RoGrid validates it before creating the project, omits the published framework
dependency and CLI pin, and maps its source directly into Roblox. Other tools
and dependencies install normally, so this is not an offline installation.

The generated Rojo configuration remembers an absolute path to your local
source. Keep that checkout in place; this mapping is specific to your machine.
Continue running the local CLI through Cargo rather than a globally installed
`rogrid`. Framework edits sync through Rojo without copying or reinstalling;
restart Play after Luau changes and restart the Cargo command after Rust changes.

Building the CLI needs a stable Rust toolchain:

```sh
cargo run -- init my-game
```

To try it from another folder without installing it:

```sh
cargo run --manifest-path path/to/rogrid/Cargo.toml -- init my-game
```

When developing from this checkout, also run its CLI for `dev`:

```sh
cargo run --manifest-path path/to/rogrid/Cargo.toml -- dev
```

Use `--local-framework` before the checkout's CLI version is released. It omits
the registry dependency and CLI pin so installation does not require a release.

`cargo fmt`, `cargo clippy` and a release build run on every push for Linux,
macOS and Windows.

## License

MIT
