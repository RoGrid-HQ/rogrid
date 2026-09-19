---
sidebar_position: 1
---

# Getting started

Create a Roblox project with generated event callers, a Rojo configuration,
and a small working example.

> RoGrid is unfinished and under active development. Breaking changes can
> occur in any release. See [project status](./status.md) for current limits.

## Prerequisites

- [Roblox Studio](https://create.roblox.com/).
- [Rokit](https://github.com/rojo-rbx/rokit#installation), available in your terminal.
- For editor support, [VS Code](https://code.visualstudio.com/) with the
  [Luau Language Server extension](https://marketplace.visualstudio.com/items?itemName=JohnnyMorganz.luau-lsp).

The steps below use Pesde. For Wally, replace `--package-manager pesde` with
`--package-manager wally`, or follow the [Wally setup guide](./package-managers.md#wally).

## 1. Install the CLI

```sh
rokit add --global RoGrid-HQ/rogrid
```

A global install makes `rogrid` available before you have a project folder.
Projects created with Rokit also pin their own CLI version.

## 2. Create a project

```sh
rogrid init my-game --package-manager pesde --tool-manager rokit
cd my-game
```

The CLI creates the folder, writes the project files, installs tools and
packages, and generates the initial event interfaces. Omit the manager flags
to choose interactively. See the [CLI reference](./cli.md) for all options.

## 3. Connect Studio

Install the Rojo Studio plugin once:

```sh
rojo plugin install
```

Start the development command:

```sh
rogrid dev
```

Open a place in Studio and connect the Rojo plugin to the running server.
Press **Play**. The starter sends the player's ready status to the server,
sets their `Ready` attribute, and prints a notification in Output.

Keep `rogrid dev` running while editing. It generates event interfaces and
runs Rojo to sync files. Restart Play after Luau changes to load the new code.
Ctrl+C stops the command and its Rojo process.

## Project structure

```text
my-game/
  default.project.json   Rojo instance mapping
  pesde.toml             Package dependencies
  rokit.toml             Tool versions
  .rogrid/generated/     Generated callers and startup code
  src/
    client/
      main.client.luau   Client startup
      ReadyDemo.luau     Example caller
      events/           Client receivers
    server/
      main.server.luau   Server startup
      events/           Server receivers
    shared/             Shared game code
```

The starter's entry scripts call `RoGrid.start()` once on each side. Put your
own game startup after that call. Remove the `ReadyDemo` call from the client
entry script when you no longer need the example.

Open the game folder in VS Code and install its recommended extension. The
starter configures Rojo sourcemaps and strict data model diagnostics so the
editor can find generated callers. Run generation before expecting completion.

## Build a place

From the project folder, generate once and then build:

```sh
rogrid dev --once
rojo build -o game.rbxl
```

Run generation before every build. `--once` still needs Rojo for its
sourcemap, but does not start a Rojo server or watcher.

Commit source, project configuration, tool manifests, and the package
lockfile. Generated files, downloaded packages, and local place files are
ignored by the starter.

## Next steps

- [Events](./events.md): send messages between the client and server.
- [Configuration](./configuration.md): change where your receivers live.
- [Troubleshooting](./troubleshooting.md): resolve setup and startup errors.
