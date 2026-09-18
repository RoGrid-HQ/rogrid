# RoGrid

The Luau runtime for RoGrid, a framework for building Roblox games with typed
client/server events. Package `0.2.0` pairs with RoGrid CLI `0.3.0`.

Use the CLI to create a project and generate its typed event callers:

```sh
rogrid init my-game
cd my-game
rogrid dev
```

The starter installs this package and starts it on both server and client with
`RoGrid.start()`. Declare receivers with `RoGrid.event(...)`; the CLI generates
the other side's `.fire(...)` and server-side `.fireAll(...)` functions.
Installing this package alone does not generate a project's event interfaces.

See the [event guide](docs/events.md) for declarations, folder configuration,
validation, and limitations, and the [project repository](https://github.com/RoGrid-HQ/rogrid)
for CLI installation and local development.

RoGrid is in early development. Expect breaking changes before version 1.0.
