# RoGrid playground

A small game for developing RoGrid without publishing or installing either
the CLI or the Luau package. The CLI runs from this checkout, and Rojo maps
`../packages/rogrid/src` directly to `ReplicatedStorage.Packages.rogrid`.
There is no copied runtime to refresh and no package installation step.

To have the CLI create a separate game with this same local framework, use
[`init --local-framework`](../README.md#create-a-separate-game-with-the-local-framework).

## Start

You need a stable Rust toolchain, Rokit, Roblox Studio, and the Rojo Studio
plugin. From the repository root:

```sh
cd playground
rokit install
rojo plugin install
cargo run --manifest-path ../Cargo.toml -p rogrid -- dev
```

Install the plugin only once. In Studio, open a place and connect the Rojo
plugin to the running server, then press Play. The client sends its ready
status to the server, which sets the player's `Ready` attribute and sends a
notification back to the clients. Look for the ready message in Studio's
Output window. That example lives in `src/client/ReadyDemo.luau`, called after
`RoGrid.start()` from the small client entry script. Remove that call when you
want only framework startup. Use a fresh place: Rojo manages the mapped script
folders.

Open **the `playground` folder** in VS Code and install the recommended Luau
Language Server extension. Its settings use this game's Rojo map, including
the framework source outside the folder. Start generation before expecting
autocomplete for event callers. You can also start the CLI with **Terminal →
Run Task → RoGrid: develop playground**.

## Editing

- Change event declarations under `src/` to try game events. The CLI regenerates
  callers automatically; Rojo syncs other game code without generation.
- To use different or multiple event folders, add `rogrid.toml` with an `[events]`
  section. See [folder configuration](../packages/rogrid/docs/events.md#declarations).
  The playground maps `src/server` and `src/client`, so folders beneath those
  roots are already included in its Rojo project. Both entry scripts simply
  call `RoGrid.start()` without repeating your folder choices.
- Change `../packages/rogrid/src/` to edit the framework. Rojo syncs those files
  directly; you do not need to rebuild Rust or reinstall a package.
- Change Rust code in `../crates/rogrid/`, then stop the command with Ctrl+C
  and run it again. Cargo rebuilds the CLI as needed.

Restart Studio Play after Luau edits so modules load the new code. This is file
sync, not hot reloading of an already running game. Ctrl+C stops the CLI and
its Rojo process. The installed global `rogrid` command is not used here.

To generate files without starting a server:

```sh
cargo run --manifest-path ../Cargo.toml -p rogrid -- dev --once
```

Commit this folder's source and configuration. Generated `.rogrid/` files,
sourcemaps, downloaded dependencies, and local Studio place files are ignored.
Keep this example small and aligned with the starter template.

This playground exercises the local source. It does not verify the published
package's contents or package-manager installation. The starter template now
targets the upcoming Luau package `0.2.0`; that package must be published before
normal generated projects can install it from the registry.
