---
sidebar_position: 2
---

# Events

Declare an event on the side that handles it. RoGrid generates a typed caller
for the other side and connects the receiver to a RemoteEvent during startup.
The examples below use the project from [getting started](./getting-started.md).

## Client to server

Create a receiver in the server events folder:

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

The first parameter must be typed `Player`. Roblox supplies the actual sender;
the client only supplies the remaining arguments.

With `rogrid dev` running, save the file. Call the generated function from
client code after `RoGrid.start()` has returned:

```luau
local Server = require(game:GetService("ReplicatedStorage").RoGridGenerated.Server)
Server.Lobby.setReady.fire(true)
```

`Lobby` comes from the module filename and `setReady` from the returned table
field. The generated client interface contains no server handler implementation.

## Server to client

Create a receiver in the client events folder:

```luau
-- src/client/events/Notifications.luau
--!strict
local RoGrid = require(game:GetService("ReplicatedStorage").Packages.rogrid)

return {
    show = RoGrid.event(function(message: string)
        print(message)
    end),
}
```

Client receivers have no automatic `Player` parameter. From server code, send
to one player or everyone:

```luau
local Client = require(game:GetService("ServerScriptService").RoGridGenerated.Client)

-- Inside a handler where player is the target Player:
Client.Notifications.show.fire(player, "You are ready!")

-- Broadcast to connected clients:
Client.Notifications.show.fireAll("A new round is starting!")
```

Use a dot for `.fire(...)` and `.fireAll(...)`. These are plain functions.

## Declarations

Event files are plain ModuleScripts directly inside an
[event folder](./configuration.md#event-folders). Name them with Luau identifiers,
such as `Lobby.luau`. Do not use `init.luau`, `.server.luau`, or `.client.luau`.

Each module returns one literal table of named `RoGrid.event(function(...) ... end)`
declarations. Use the local name `RoGrid` and inline functions so the generator
can recognize them. Keep helper modules outside event folders; handlers can
require helpers and use ordinary Luau.

Every parameter needs an explicit type. Payloads support primitives,
Roblox values and Instance references, records, arrays, dictionaries, optionals,
unions, and local or imported aliases. There are at most 16 payload arguments per event. The automatic
server `Player` does not count toward this limit. See the
[API reference](./api.md#declaration-rules) for the complete rules.

## Startup

The starter already calls `RoGrid.start()` once on the server and once on each
client. It loads the receivers and connects their events. There is no folder
list to repeat in your startup scripts.

Do not fire events at receiver module top level: those modules are loaded
during startup. Fire inside handlers or after `RoGrid.start()` returns.

Events send messages without returning replies. Sending does not acknowledge
that a handler ran. Messages sent before a client finishes loading are not
persistent state; arrange your game's initial state exchange accordingly.

## Validation

Generated descriptors drive a shared validator that checks argument types,
nested shapes, finite components, sizes, and a per-message work limit.
Invalid payloads are dropped before the handler runs. RoGrid does not impose
an event rate limit.

These checks do not authorize game actions. Your handler still checks things
such as ownership, range, purchase permissions, and gameplay cooldowns. See
[runtime validation](./api.md#runtime-validation) for exact limits and error behavior.

## Editing events

`rogrid dev` regenerates callers when declarations change. It reports `+` for
added events, `-` for removed events, and `~` for changed argument names or
types. Handler-body edits do not appear in that report.

Fix generation errors before starting Play. Restart Play after changes so
Roblox reloads modules. For a place build, run `rogrid dev --once` first.
Generated files under `.rogrid/generated/` are owned by RoGrid; edit the
receiver source instead.
