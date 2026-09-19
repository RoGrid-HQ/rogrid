# RoGrid

Typed client/server events for Roblox. Declare a receiver in Luau, and the
RoGrid CLI generates its callers, payload validation, and startup wiring.
This package supplies the runtime used by that generated code.

> **Development status:** RoGrid is unfinished and under active development.
> Breaking changes can occur in any release. Review changes before upgrading.

## Getting started

With [Rokit](https://github.com/rojo-rbx/rokit#installation) and Roblox Studio
installed, create a project:

```sh
rokit add --global RoGrid-HQ/rogrid
rogrid init my-game --package-manager pesde --tool-manager rokit
cd my-game
rojo plugin install
rogrid dev
```

Connect the Rojo plugin in Studio and press **Play**. The starter installs this
package as `ReplicatedStorage.Packages.rogrid` and calls `RoGrid.start()` on
both sides. Installing the runtime package alone does not generate callers
or add startup scripts to a game.

## Example

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

After startup, the client can call:

```luau
local Server = require(game:GetService("ReplicatedStorage").RoGridGenerated.Server)
Server.Lobby.setReady.fire(true)
```

Server handlers receive the sending `Player` automatically. Payload arguments
support `string`, `boolean`, and `number`. Client receivers generate server
callers with `.fire(player, ...)` and `.fireAll(...)`.

## Documentation

These guides are included in the package for Pesde and can also be read in
the repository:

- [Getting started](https://github.com/RoGrid-HQ/rogrid/blob/main/packages/rogrid/docs/getting-started.md)
- [Event guide](https://github.com/RoGrid-HQ/rogrid/blob/main/packages/rogrid/docs/events.md)
- [Configuration](https://github.com/RoGrid-HQ/rogrid/blob/main/packages/rogrid/docs/configuration.md)
- [CLI reference](https://github.com/RoGrid-HQ/rogrid/blob/main/packages/rogrid/docs/cli.md)
- [API reference](https://github.com/RoGrid-HQ/rogrid/blob/main/packages/rogrid/docs/api.md)
- [Troubleshooting](https://github.com/RoGrid-HQ/rogrid/blob/main/packages/rogrid/docs/troubleshooting.md)
- [Package managers](https://github.com/RoGrid-HQ/rogrid/blob/main/packages/rogrid/docs/package-managers.md)
- [Project status](https://github.com/RoGrid-HQ/rogrid/blob/main/packages/rogrid/docs/status.md)

The runtime is published as `rogrid/rogrid` on Pesde and `rogrid-hq/rogrid` on
Wally. The CLI pins the matching runtime version when creating a project.
Use `--package-manager wally` during `init` to choose Wally.

## License

[MIT](https://github.com/RoGrid-HQ/rogrid/blob/main/packages/rogrid/LICENSE)
