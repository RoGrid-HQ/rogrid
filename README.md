# RoGrid

[![CI](https://github.com/RoGrid-HQ/rogrid/actions/workflows/ci.yml/badge.svg)](https://github.com/RoGrid-HQ/rogrid/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/RoGrid-HQ/rogrid)](https://github.com/RoGrid-HQ/rogrid/releases/latest)

RoGrid is a Luau framework and CLI for Roblox games. Define typed events and
requests in Luau, and RoGrid generates callers and payload validation.
The CLI sets up your project, installs dependencies, and runs Rojo while you
develop.

> **Development status:** RoGrid is unfinished and under active development.
> Breaking changes can occur in any release. Review changes before upgrading.

## Getting started

Install [Roblox Studio](https://create.roblox.com/) and
[Rokit](https://github.com/rojo-rbx/rokit#installation), then run:

```sh
rokit add --global RoGrid-HQ/rogrid
rogrid init my-game --package-manager pesde --tool-manager rokit
cd my-game
rojo plugin install
rogrid dev
```

Install the Rojo plugin once. Open a place in Studio, connect with the Rojo
plugin, and press **Play**. The starter requests the player's current ready
status, sends a ready event, and prints a notification in Output.

RoGrid supports Pesde and Wally. To use Wally, replace `--package-manager pesde`
with `--package-manager wally`. See
[package managers](packages/rogrid/docs/package-managers.md) for setup options.

## Typed events

Declare an event where it is handled:

```luau
-- src/server/events/Lobby.luau
--!strict
local RoGrid = require(game:GetService("ReplicatedStorage").Packages.rogrid)

return {
    setReady = RoGrid.event(function(player: Player, ready: boolean)
        player:SetAttribute("Ready", ready)
    end),
}
```

Call it from the client after `RoGrid.start()`:

```luau
local Server = require(game:GetService("ReplicatedStorage").RoGridGenerated.Server)
Server.Lobby.setReady.fire(true)
```

Roblox supplies the sending `Player`. The CLI generates argument types and
payload validation from the handler. Client receivers work the same way, with
server callers for one player or all players.

Payloads support Roblox values and Instance references, nested tables,
optionals, unions, and shared type aliases. Events support reliable delivery
by default and `UnreliableRemoteEvent` with `{ reliability = "unreliable" }`.
Read the [event guide](packages/rogrid/docs/events.md) for both directions.

## Typed requests

Use a request when the client needs a server result. Add a handler to the
server module's returned table:

```luau
getReady = RoGrid.request(function(player: Player): boolean
    return player:GetAttribute("Ready") == true
end),
```

Call it after client startup, with an optional timeout as the final argument:

```luau
local ok, ready = pcall(Server.Lobby.getReady.invoke, 5)
if ok then
    print("Ready:", ready)
else
    warn(tostring(ready))
end
```

The caller yields for typed results. Omitting the timeout leaves the call
without a deadline. Read the [request guide](packages/rogrid/docs/requests.md)
for arguments, multiple return values, and failures.

## Documentation

| Page | What you will learn |
| --- | --- |
| [Getting started](packages/rogrid/docs/getting-started.md) | Create a project, connect Studio, and build a place. |
| [Events](packages/rogrid/docs/events.md) | Declare receivers and call them from either side. |
| [Requests](packages/rogrid/docs/requests.md) | Return typed server results with optional per-call timeouts. |
| [Configuration](packages/rogrid/docs/configuration.md) | Choose event folders and map them with Rojo. |
| [Package managers](packages/rogrid/docs/package-managers.md) | Choose Pesde or Wally and configure tools. |
| [CLI reference](packages/rogrid/docs/cli.md) | Commands, flags, and local framework setup. |
| [API reference](packages/rogrid/docs/api.md) | Runtime functions, generated callers, and validation. |
| [Troubleshooting](packages/rogrid/docs/troubleshooting.md) | Fix installation, generation, and startup errors. |
| [Project status](packages/rogrid/docs/status.md) | Check release availability and current limitations. |

The guides live in `packages/rogrid/docs/` and are included when publishing
the runtime package to Pesde.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the repository layout, local
development, tests, and documentation conventions. The
[playground](playground/README.md) runs directly from source.

## License

[MIT](packages/rogrid/LICENSE)
