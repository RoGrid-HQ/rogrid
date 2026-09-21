---
sidebar_position: 3
---

# Requests

Use a request when client code needs a result from a server handler. Declare
the handler with `RoGrid.request`; RoGrid generates an `.invoke(...)` caller
with its argument and return types. The calling thread yields until a reply
arrives, the call fails, or a timeout supplied by the caller expires.

## Declare a server receiver

Requests live alongside events in the configured server receiver folders.
For example, add a request to `src/server/events/Lobby.luau`:

```luau
--!strict
local RoGrid = require(game:GetService("ReplicatedStorage").Packages.rogrid)

return {
    getReady = RoGrid.request(function(player: Player): boolean
        return player:GetAttribute("Ready") == true
    end),
}
```

The first parameter must be `Player`. Roblox supplies the actual sender;
the client sends only the remaining arguments. Annotate every parameter and
the return type. Local and imported aliases use the same
[payload types](./api.md#payload-types) as events.

## Call a request

After `RoGrid.start()` has returned on the client:

```luau
local Server = require(game:GetService("ReplicatedStorage").RoGridGenerated.Server)

local ready = Server.Lobby.getReady.invoke()  -- boolean; no timeout
local readyWithinFiveSeconds = Server.Lobby.getReady.invoke(5)
```

The timeout is an optional **final argument**, measured in seconds. It is
handled by RoGrid and is not passed to the server handler. Each invocation
chooses its own timeout; there is no default deadline or timeout setting in
project configuration.

For a handler with payload arguments, put the timeout after every declared
payload position. For example, a handler declared as
`function(player: Player, itemId: string, note: string?): Item?` is called with:

```luau
Server.Inventory.find.invoke("Sword")          -- No note or timeout.
Server.Inventory.find.invoke("Sword", nil, 5)  -- No note; five-second timeout.
```

Use `nil` placeholders for omitted optional payload arguments when supplying
a timeout. A numeric optional payload argument still occupies its declared
position; it is not interpreted as the timeout.

Timeouts must be finite, non-negative numbers. A timeout of zero fails
immediately without sending the request. With no timeout, a handler that
never completes leaves the caller waiting. Startup dependency waits are
separate from request timeouts.

A timeout stops the caller from waiting. It does not cancel the server
handler or undo game changes. Late replies are ignored. RoGrid does not retry
requests automatically; account for possible completed work before retrying
an operation that changes game state.

## Return values

Handlers can return one value, a fixed tuple such as `(boolean, string?)`,
or `()` to acknowledge completion without returning values. Optional values
and trailing `nil` results retain their positions. Variadic return types and
generic handlers are unsupported.

RoGrid validates arguments before calling the handler, validates results
before sending them, and validates the received reply. Roblox's
[remote argument limitations](https://create.roblox.com/docs/scripting/events/remote#argument-limitations)
also apply to return values. For example, an Instance that is not visible to
the client can arrive as `nil`, causing a required Instance result to fail
validation.

Concurrent requests have independent results. Handlers may yield, and replies
can complete in a different order from the calls.

## Handle failures

Use `pcall` when calling a request that can fail:

```luau
local ok, result = pcall(Server.Lobby.getReady.invoke, 5)
if ok then
    print("Ready:", result)
elseif typeof(result) == "table" and result.code == "Timeout" then
    warn("The server did not reply before the deadline.")
else
    warn(tostring(result))
end
```

Request failures raise a `RoGrid.RequestError` with `code` and `endpoint`
fields. `tostring(error)` includes both fields.

| Code | Meaning |
| --- | --- |
| `Timeout` | The caller's deadline expired. |
| `InvalidRequest` | The receiver rejected the request arguments. |
| `InvalidResponse` | The handler results or the received reply did not match the contract. |
| `HandlerError` | The server handler raised an error. |
| `SendFailed` | The request could not be sent. |
| `Disconnected` | The connection's player departed while the request was pending. |

Handler errors are logged on the server with the endpoint and traceback.
The client receives a failure code rather than the server traceback. Calls
made on the wrong side, before startup, with an invalid timeout, or through
stale generated code raise ordinary Luau errors.

Expected gameplay outcomes, such as an unavailable item, belong in the
handler's return type. A tagged union such as
`{ok: true, itemId: string} | {ok: false, reason: string}` lets callers handle
those outcomes using normal typed code.

## Game policies

RoGrid supplies typed communication and payload validation. Your game
controls authorization, cooldowns, throttling, and concurrent work. The
framework adds no argument-count, payload-size, or concurrency limits.
Roblox and Luau platform constraints still apply.

Requests use reliable `RemoteEvent` transport and currently run from client
to server. Use [events](./events.md) for server-to-client notifications and
unreliable updates.
