---
sidebar_position: 6
---

# API reference

Require the runtime from `ReplicatedStorage.Packages.rogrid`. The starter
installs it and supplies server and client startup scripts. For a walkthrough,
see [events](./events.md).

## RoGrid.event

```luau
RoGrid.event<A...>(handler: (A...) -> ()): (A...) -> ()
```

Marks an inline handler for the CLI's event generator. At runtime it returns
the supplied function. Calling `RoGrid.event` alone does not create or
connect a RemoteEvent; the CLI must generate the project's event code.

### Declaration rules

- Put plain modules directly in configured event folders. Each module must
  return one literal table with at least one named event field.
- Use `name = RoGrid.event(function(...) ... end)` directly. Renaming `RoGrid`,
  passing a function reference, or computing the returned table is unsupported.
- Annotate every parameter. A server handler's first parameter must be
  `Player`; it is supplied by Roblox rather than sent in the payload.
- Payload types are exactly `string`, `boolean`, and `number`. Aliases,
  optional types, unions, tables, Instances, and variadic arguments are unsupported.
- A handler can have at most 16 payload arguments, excluding the server's
  first `Player`. Generic handlers are unsupported.
- Event names and parameter names must be unique within their respective
  table or handler. Parameter names `self` and those beginning `_rogrid` are reserved.
- Omit the return annotation or use `()`. Events do not return a reply.

## RoGrid.start

```luau
RoGrid.start(): ()
```

Starts the generated code for the current side, loads receivers, and connects
their RemoteEvents. Call once on the server and once on each client, before
firing events. Startup fails if generation is missing, invalid, incompatible
with the runtime, or inconsistent between the generated files.

Server startup:

```luau
local RoGrid = require(game:GetService("ReplicatedStorage").Packages.rogrid)
RoGrid.start()
```

Client startup waits for the package to replicate:

```luau
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local Packages = ReplicatedStorage:WaitForChild("Packages")
local RoGrid = require(Packages:WaitForChild("rogrid"))
RoGrid.start()
```

Do not fire events at receiver module top level while startup is loading
those modules. Use handlers or game code that runs after startup returns.

## RoGrid.version

```luau
RoGrid.version: string
```

The installed runtime's version string. The CLI has its own version and pins
the matching runtime when creating a project. See [project status](./status.md)
for compatibility and upgrade guidance.

## Generated callers

The CLI derives module names, event names, and argument types from receivers.
Callers use ordinary dot calls and return `()`.

| Caller | Where to call it | Arguments |
| --- | --- | --- |
| `Server.Module.event.fire(...)` | Client | Receiver payload arguments, excluding the automatic `Player`. |
| `Client.Module.event.fire(player, ...)` | Server | Target `Player`, then receiver payload arguments. |
| `Client.Module.event.fireAll(...)` | Server | Receiver payload arguments, sent to all connected clients. |

Require `Server` from `ReplicatedStorage.RoGridGenerated.Server` and `Client`
from `ServerScriptService.RoGridGenerated.Client`. Calling from the wrong side,
before startup, or through a stale generated caller raises an error.

Each declaration uses one ordinary RemoteEvent. Callers do not return a
response or acknowledge handler completion. There is no persistence,
automatic retry, or client readiness protocol.

## Runtime validation

Validation runs on the receiving side before the game handler:

| Check | Limit or behavior |
| --- | --- |
| Argument count | Must exactly match the declaration. |
| Argument types | Must match each declared payload type. |
| Numbers | Must be finite; NaN and infinities are rejected. |
| Strings | At most 4096 bytes each. |
| Client event rate | Per player, a burst of 60 with a refill of 60 per second, shared across all inbound events. |

Invalid or excessive client messages are silently dropped by the server.
The client warns about invalid server payloads and skips the handler.
Rate limiting and type checks do not replace game-specific authorization
or cooldowns.

Handler errors are logged on the receiving side with the event name and
traceback. They are not returned to the sender. RoGrid does not cancel
handlers that yield indefinitely.
