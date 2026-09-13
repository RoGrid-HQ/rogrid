# RoGrid

A framework for building Roblox games from your editor.

```
rokit add --global RoGrid-HQ/rogrid
rogrid new my-game
cd my-game
rogrid dev
```

> v0.1, early. The CLI and starter project work. The runtime library is in progress. Expect breaking changes until 1.0.

## Install

You need [Rokit](https://github.com/rojo-rbx/rokit) and Roblox Studio.

```
rokit add --global RoGrid-HQ/rogrid
```

Then install the Rojo Studio plugin once, from inside any RoGrid project:

```
rojo plugin install
```

## Quick start

```
rogrid new my-game
cd my-game
rogrid dev
```

Open a new Baseplate in Studio, connect with the Rojo plugin, press Play. `HelloService` and `HelloController` print in the Output window. Edit a file on disk and it syncs into Studio.

## Project layout

```
my-game/
  rokit.toml              pinned tool versions
  default.project.json    maps src/ into the Roblox tree
  src/
    server/services/      server modules, started automatically
    client/controllers/   client modules, started automatically
    shared/               modules both sides can require
```

A service or controller is a module with a `Start()` method. Drop a file in the folder and it runs.

```lua
local CoinsService = {}

function CoinsService:Start()
	print("CoinsService started")
end

return CoinsService
```

## Commands

| Command | What it does |
| --- | --- |
| `rogrid new <name>` | Creates a project, runs `git init` and `rokit install` |
| `rogrid dev` | Starts the Rojo server for the current project |
| `rogrid generate service <name>` | Creates a server service with `Init()` and `Start()` |
| `rogrid generate controller <name>` | Creates a client controller with `Init()` and `Start()` |
| `rogrid generate module <name>` | Creates a shared module |

## Generators

Run generators from your project folder or any folder inside it.

```sh
rogrid generate service Inventory    # src/server/services/InventoryService.luau
rogrid generate controller Camera    # src/client/controllers/CameraController.luau
rogrid generate module Currency      # src/shared/Currency.luau
```

Services and controllers are picked up automatically by the runtime. Their `Init()` methods run before any `Start()` methods on the same side (server or client). Shared modules return a plain table and must be required where needed.

The `Service` and `Controller` suffixes are added only if missing, so `InventoryService` also works. Names must start with an ASCII letter or `_` and contain only ASCII letters, numbers, or `_`; keywords and reserved filenames are rejected. Pass a name without a file extension or nested path. Existing files are never overwritten.

## Contributing

```
crates/rogrid/       the CLI (Rust)
templates/default/   the starter project, embedded into the CLI
templates/generators/ Luau templates used by generate, also embedded into the CLI
```

Build locally with `cargo run -- new demo`.

## License

MIT
