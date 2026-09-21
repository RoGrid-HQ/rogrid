---
sidebar_position: 7
---

# API reference

Require the runtime from `ReplicatedStorage.Packages.rogrid`. The starter
installs it and supplies server and client startup scripts. For a walkthrough,
see [events](./events.md) and [requests](./requests.md).

## RoGrid.event

```luau
type EventOptions = { reliability: ("reliable" | "unreliable")? }
RoGrid.event<A...>(handler: (A...) -> (), options: EventOptions?): (A...) -> ()
```

Marks an inline event handler for the CLI. At runtime it returns the supplied
function. Reliability defaults to `"reliable"`, using a `RemoteEvent`.
Pass `{ reliability = "unreliable" }` as a literal options table to use an
`UnreliableRemoteEvent`. See [unreliable events](./events.md#unreliable-events).

## RoGrid.request

```luau
RoGrid.request<A..., R...>(handler: (A...) -> R...): (A...) -> R...
```

Marks an inline server request handler for the CLI and returns the supplied
function at runtime. Its generated `.invoke(...)` caller yields and returns
the handler's typed results. The handler must declare a single return type,
a fixed tuple such as `(boolean, string?)`, or `()` for an acknowledgement.
Requests use reliable `RemoteEvent` transport. See [requests](./requests.md)
for timeout and error handling.

Neither declaration function creates or connects remotes by itself. Generate
the project's code with the CLI and call `RoGrid.start()` on both sides.

## Declaration rules

- Put plain modules directly in configured event folders. Each module must
  return one literal table with at least one named event or request field.
- Use `name = RoGrid.event(function(...) ... end)` or
  `name = RoGrid.request(function(...) ... end)` directly. Renaming `RoGrid`,
  passing a function reference, or computing the returned table is unsupported.
- Annotate every parameter. A server handler's first parameter must be
  `Player`; it is supplied by Roblox rather than sent in the payload.
- Use the explicit [payload types](#payload-types) below. Variadic arguments
  and unvalidated types such as `any` are unsupported.
- Generic handlers are unsupported. RoGrid imposes no separate cap on the
  number of payload arguments.
- Endpoint names and parameter names must be unique within their respective
  table or handler. Parameter names `self` and those beginning `_rogrid` are reserved.
- For events, omit the return annotation or use `()`. Events do not return a reply.
- For requests, explicitly annotate the return type using the payload types
  below. Variadic return types are unsupported. Request declarations are
  supported in server receiver folders only.

## Payload types

| Type | Examples |
| --- | --- |
| Primitives | `string`, `number`, `boolean`, `buffer`, `nil` |
| Optional values | `string?`, `{Vector3}?` |
| Dense arrays | `{Vector3}` or `{[number]: Vector3}` |
| String dictionaries | `{[string]: number}` |
| Records | `{id: string, count: number, note: string?}` |
| Literals and unions | `"equip"`, `false`, `string \| number`, `{kind: "equip", id: string} \| {kind: "clear"}` |
| Instance references | `Instance`, `Model`, `BasePart`, `TextLabel`, and known Roblox subclasses |
| Enum values | `EnumItem`, `Enum.Material`, `Enum.Font`, and known enum families |
| Geometry | `Vector2`, `Vector3`, `Vector2int16`, `Vector3int16`, `CFrame`, `UDim`, `UDim2`, `Rect`, `Ray`, `Region3`, `Region3int16` |
| Appearance and other values | `Color3`, `BrickColor`, `Font`, `NumberRange`, `NumberSequence`, `NumberSequenceKeypoint`, `ColorSequence`, `ColorSequenceKeypoint`, `DateTime`, `Axes`, `Faces`, `PhysicalProperties` |

These rules concern network payloads only. They do not limit types in ordinary
game code or future framework features.

Tables may nest. Arrays must be dense, with no nil elements or extra keys;
`{T?}` is a generation error, including through aliases. Use `{T}?` to make the
whole array optional. Dictionaries accept only string keys. Records reject
unknown fields, even though Luau's structural typing permits wider tables.
Pass the declared fields explicitly when sending part of a larger object.

Instances cross as references to objects visible to the receiver. RoGrid does
not clone Models, replicate client-created Instances, or transfer a UI tree.
Roblox may deliver an invisible reference as nil; a required Instance is then
rejected. Class checks do not verify ownership or location. See Roblox's
[remote argument limitations](https://create.roblox.com/docs/scripting/events/remote#argument-limitations).

### Aliases

Use non-generic, non-recursive aliases at module top level:

```luau
type Item = {id: string, position: Vector3?}

return {
    equip = RoGrid.event(function(player: Player, item: Item, note: string?)
        -- Check ownership here.
    end),
}
```

Shared modules can declare `export type Item = ...`. Import them with a static
top-level require, then annotate `Types.Item`:

```luau
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local Types = require(ReplicatedStorage.Shared.Types)
```

The generator follows Rojo mappings for `game`/`script` paths, dot or string
indexing, `GetService`, `WaitForChild` (including a timeout), and `FindFirstChild`
without recursive search. Static locals can hold intermediate paths; RoGrid
does not cap the number of local references followed and rejects cycles. String
requires support `./`, `../`, `@self/`, and `@game/`, following Roblox Instance
names from the sourcemap, including renamed modules. See the
[Roblox require rules](https://create.roblox.com/docs/reference/engine/globals/LuaGlobals#require).
Imported files must remain inside the project, outside `.git` and `.rogrid`.
Do not rebind import/path locals
or shadow `require`, `game`, or `script`. Imported aliases must be exported.
Aliases expand into concrete caller types, so callers do not import server modules.
Shared and transitive type edits change the generated revision.

Unsupported: generic or recursive aliases, computed `typeof` types,
intersections, read/write type modifiers, type packs, functions, threads, signals, arbitrary userdata,
`any`, `unknown`, and untyped `table`. Tables cannot combine record fields and
an indexer, use metatables, or contain cycles. Engine types outside the list,
such as `TweenInfo`, `RaycastResult`, and raycast/overlap parameters, need an
explicit record containing the data you want to send. Alias names must not
shadow built-in payload types. Record names and import paths must be UTF-8;
payload strings can contain arbitrary bytes.

RoGrid imposes no fixed depth or expansion-size cap on payload types or alias
chains. Larger expanded types require more generation time and memory.
Recursive aliases are rejected; Luau's own parsing and compilation limits
still apply.

## RoGrid.start

```luau
RoGrid.start(): ()
```

Starts the generated code for the current side, loads receivers, and connects
their remotes. Call once on the server and once on each client, before
sending events or requests. Startup waits for required modules and remotes without a
timeout. If an object never appears, startup keeps waiting. Roblox's built-in
[`WaitForChild` warning](https://create.roblox.com/docs/reference/engine/classes/Instance#WaitForChild)
reports waits longer than five seconds without stopping them.

Invalid generation, incorrect object classes, incompatible CLI/runtime
protocols, and inconsistent generated revisions still raise errors.

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

Do not send events or requests at receiver module top level while startup is loading
those modules. Use handlers or game code that runs after startup returns.

## RoGrid.version

```luau
RoGrid.version: string
```

The installed runtime's version string. The CLI has its own version and pins
the matching runtime when creating a project. See [project status](./status.md)
for compatibility and upgrade guidance.

## Generated callers

The CLI derives module names, endpoint names, argument types, and request
return types from receivers. Callers use ordinary dot calls.

| Caller | Where to call it | Arguments |
| --- | --- | --- |
| `Server.Module.event.fire(...)` | Client | Receiver payload arguments, excluding the automatic `Player`. |
| `Client.Module.event.fire(player, ...)` | Server | Target `Player`, then receiver payload arguments. |
| `Client.Module.event.fireAll(...)` | Server | Receiver payload arguments, sent to all connected clients. |
| `Server.Module.request.invoke(..., timeoutSeconds?)` | Client | Receiver payload arguments, excluding `Player`, then an optional timeout in seconds. |

Require `Server` from `ReplicatedStorage.RoGridGenerated.Server` and `Client`
from `ServerScriptService.RoGridGenerated.Client`. Calling from the wrong side,
before startup, or through a stale generated caller raises an error.

Event callers return `()` without waiting for handler completion. Each event
uses one `RemoteEvent` or `UnreliableRemoteEvent`, according to its declaration.
Request callers yield for the handler's declared results; each request uses
one reliable `RemoteEvent` for calls and replies. Supplying a timeout requires
all declared payload positions, including `nil` placeholders for omitted
optional arguments. Omitting the timeout leaves the call waiting without a
deadline. See [request calls](./requests.md#call-a-request).

RoGrid provides no persistence, automatic retries, or client readiness protocol.

## RoGrid.RequestError

Request failures raise an error table with `code` and `endpoint` fields.
`tostring(error)` gives a readable message. Handle failures with `pcall`;
see the [error codes and examples](./requests.md#handle-failures).

Programming errors, such as invoking before startup or passing an invalid
timeout, raise ordinary Luau errors.

## Runtime validation

Validation runs on incoming arguments before the game handler. Requests also
validate the handler's results before sending and again on receipt:

| Check | Behavior |
| --- | --- |
| Argument count | Extra arguments are rejected. Omitted values are checked as nil, so trailing optional arguments may be omitted. |
| Argument types | Must match each declared payload type. |
| Numbers | Must be finite, including numeric components of Roblox value types. |

Invalid event payloads are dropped before the handler runs. Invalid request
arguments fail with `InvalidRequest`; invalid results fail with
`InvalidResponse`. In Studio, invalid incoming payloads produce a warning
with the endpoint name and full field path, including repeated failures. For
a union, the warning includes the first branch's failure. Invalid handler
results are logged on the server.

RoGrid does not impose string or buffer size caps, a validation step budget,
or rate or concurrency limits. Payload size policies, authorization, and cooldowns
belong in your game code. Validation visits table contents and may try multiple
union branches, so larger payloads can require more work. Roblox has already
deserialized the message when validation runs, and handler checks happen afterward.

Handler errors are logged on the receiving side with the endpoint name and
traceback. Event senders receive no reply; request callers receive
`HandlerError` without the traceback. RoGrid does not cancel handlers that
yield indefinitely, including when a request's caller times out.
