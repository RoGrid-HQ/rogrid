---
sidebar_position: 7
---

# Troubleshooting

Run commands from the game folder containing `default.project.json`. When
using a local framework, run the same checkout's CLI for both `init` and `dev`.

## A tool cannot be found

Check `rojo --version` in the game folder. Generation uses Rojo even with
`rogrid dev --once`.

For Rokit projects, run `rokit install`. For Pesde-provided tools, put
`~/.pesde/bin` on PATH and open a new terminal. If another Rojo installation
comes first, adjust PATH to use the intended version. With Wally and no tool
manager, install Wally and Rojo yourself before running `init`.

## Init wrote files but installation failed

The CLI prints the failed command and remaining install steps.
Fix the reported tool, network, or package error and run those commands in
the created folder. Then run:

```sh
rogrid dev --once
```

If only generation failed, fix its reported error and run the same command.
The project files remain available; rerunning `init --force` would overwrite
starter files you may have edited.

## Wally cannot find RoGrid

Check that `wally.toml` names the dependency `rogrid-hq/rogrid`, keeps the
alias `rogrid`, and uses `https://github.com/UpliftGames/wally-index` as its
registry. Keep the exact runtime version written by the CLI, then run
`wally install` from the game folder. Check the error for a network failure
or an unavailable version.

If `rogrid init --help` does not list `wally`, update the CLI. See
[package managers](./package-managers.md#wally) for the complete setup.

## Generation rejects a receiver

Read the file and line in the CLI error. Check the
[declaration rules](./api.md#declaration-rules), especially inline handlers,
explicit parameter types, and the first `Player` parameter on server events.
Keep helper modules outside event folders.

For missing folders or ambiguous ModuleScripts, check
[event folders and Rojo mapping](./configuration.md). Every receiver must map
exactly once to a supported location.

## Studio reports missing or stale generated files

Run `rogrid dev --once` successfully, reconnect Rojo if needed, and restart
Play. Check the generated mappings in `default.project.json` and the installed
runtime at `ReplicatedStorage.Packages.rogrid`.

Generated code and the runtime must use the same internal protocol. Install a
compatible CLI/runtime pair, regenerate, and restart Play. Running generation
does not update installed packages. For framework development, use the same
checkout's CLI and runtime through `--local-framework` or the playground.

## A payload is dropped

In Studio, check Output for `RoGrid dropped` followed by the event and field
path. Verify required fields, array density, and the
[payload rules and limits](./api.md#runtime-validation). Records reject extra
fields even when Luau accepts the wider table type. Instance references must
be visible to the receiver. Repeated warnings are throttled to once per second
per event.

## Caller types are missing in the editor

Open the game folder in VS Code, install its recommended Luau Language
Server extension, and run generation. Check the
[editor settings](./configuration.md#editor-settings) and use `--!strict` in
your modules. Existing settings are preserved by `dev`, so they may need
updating manually.

## Saved changes do not affect a running game

Restart Studio Play after Luau changes. Rojo syncs files, but RoGrid does
not reload cached modules in a running game. For local CLI development,
restart the Cargo command after Rust changes.
