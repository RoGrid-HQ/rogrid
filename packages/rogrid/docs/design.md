# Design: the client-server boundary

Status: proposal. Nothing on this page is built yet. It describes what the
first real version of RoGrid does and, just as importantly, what it leaves alone.

## Summary

You write a normal typed Luau function in a folder. `rogrid dev` reads its type
annotations and generates the code that makes it callable from the client:
an input validator, a rate limit, and a typed client module. Your Luau types are
the contract. There is no schema language and there are no remotes to create.

RoGrid owns the boundary between server and client and nothing else. It does not
load your modules, own a lifecycle, or choose your state, UI, or data libraries.

## What RoGrid is not

It is not a compiler. Your files go into the game exactly as written, synced by
Rojo. RoGrid reads them and writes additional files next to them. What you type
is what runs. The closest comparison is `prisma generate` or Convex's
`_generated` folder, not roblox-ts.

It requires the editor-first workflow that Rojo already requires. Adding a
function or changing its input or output type needs the CLI. Editing a handler's
body does not.

## Two primitives

| Direction        | Primitive | Declared in                     |
| ---------------- | --------- | ------------------------------- |
| Client to server | Function  | `src/server/functions/**.luau`  |
| Server to client | Event     | `src/shared/events.luau`        |

There is no server-to-client request with a response. Waiting on a client is
unsafe, so the design leaves it out.

## Functions

One file per function. The path below `functions/` is its name, so
`functions/shop/buyItem.luau` becomes `api.shop.buyItem`. Everything a client can
call lives in that one folder, which makes the attack surface reviewable with `ls`.

```lua
--!strict
-- src/server/functions/shop/buyItem.luau
local RoGrid = require("@game/ReplicatedStorage/Packages/rogrid")
local Shop = require("../../systems/Shop")

type Input = {
	itemId: string,
	quantity: number?,
}

type Output = {
	balance: number,
}

return RoGrid.fn(function(player: Player, input: Input): Output
	local balance = Shop.buy(player, input.itemId, input.quantity or 1)
	return { balance = balance }
end)
```

Rules:

- The module returns exactly one `RoGrid.fn(...)`.
- The handler's first parameter is `player: Player`.
- The second parameter, if present, is a single input table and must be
  annotated. Functions never take positional arguments. A function with no
  input omits the parameter.
- The names `Input` and `Output` are a convention. The generator reads the
  handler's signature, not the alias names.
- Files and folders starting with `_` are not functions.

### Calling from the client

```lua
local api = require("@game/ReplicatedStorage/RoGrid/api")

local result = api.shop.buyItem({ itemId = "sword", quantity = 2 })
print(result.balance)
```

The call yields and returns `Output`. The input has already been validated on
the server before the handler ran.

### Request and response, or one-way

- A handler with a return type annotation is request and response. The client
  call yields until the result arrives or the call times out.
- A handler with no return type annotation is one-way. The client call returns
  immediately. Returning a value from such a handler is a generation error, so
  forgetting the annotation cannot silently change behaviour.

```lua
-- src/server/functions/combat/aim.luau
type Input = { direction: Vector3 }

return RoGrid.fn({ unreliable = true }, function(player: Player, input: Input)
	Aim.update(player, input.direction)
end)
```

### Options

`RoGrid.fn(handler)` or `RoGrid.fn(options, handler)`. Options must be a literal
table so the generator can read them.

| Option       | Meaning                                                        |
| ------------ | -------------------------------------------------------------- |
| `guards`     | List of guard functions, run in order before the handler       |
| `limit`      | Calls per second, per player. Overrides the default            |
| `timeout`    | Seconds before a request fails on the client                   |
| `unreliable` | Send over an unreliable remote. One-way functions only         |

### Failures

Expected failures belong in the output type, so the client is forced to handle
them and gets autocomplete on the reasons.

```lua
type Output =
	{ status: "ok", balance: number }
	| { status: "failed", reason: "unknown-item" | "insufficient-funds" }
```

```lua
local result = api.shop.buyItem({ itemId = "sword" })
if result.status == "ok" then
	print(result.balance)
else
	warn(result.reason)
end
```

Unexpected failures raise an error on the client, the way `InvokeServer` does.
`RoGrid.try` turns that into a typed result for callers that want to react.

```lua
local call = RoGrid.try(api.shop.buyItem, { itemId = "sword" })
if call.ok then
	print(call.value.balance)
else
	warn(call.code)
end
```

Error codes:

| Code            | Cause                                                       |
| --------------- | ----------------------------------------------------------- |
| `rate-limited`  | The player exceeded the function's limit                    |
| `invalid-input` | The input failed validation                                 |
| `timeout`       | No response arrived in time                                 |
| `internal`      | The handler raised an error. Details stay on the server     |
| anything else   | The reason string returned by the guard that refused        |

## Events

One file, types only. Each key is an event and its value is the payload type.

```lua
--!strict
-- src/shared/events.luau
local RoGrid = require("@game/ReplicatedStorage/Packages/rogrid")

export type Events = {
	coinsChanged: { coins: number },
	roundStarted: { map: string, endsAt: number },
	hit: RoGrid.Unreliable<{ position: Vector3, damage: number }>,
}

return {}
```

Each side gets its own generated view with only the methods that are valid
there, so misuse is a type error and not a runtime error.

```lua
-- server
local events = require("@game/ServerScriptService/RoGrid/events")

events.coinsChanged:fire(player, { coins = 120 })
events.roundStarted:fireAll({ map = "Crossroads", endsAt = os.time() + 300 })
events.hit:fireExcept(attacker, { position = position, damage = 25 })
events.hit:fireList(nearbyPlayers, { position = position, damage = 25 })
```

```lua
-- client
local events = require("@game/ReplicatedStorage/RoGrid/events")

local disconnect = events.coinsChanged:on(function(payload)
	coinsLabel.Text = tostring(payload.coins)
end)
```

Event payloads are not validated on the client. The client trusts the server,
and static types already check every `fire` call.

## Guards

A guard is a plain function that says whether a call may proceed. It returns
`true`, or `false` and a reason. The reason becomes the error code on the client.
There is no wrapper and nothing to import. A guard that needs settings is a
function that returns a guard.

```lua
-- src/server/guards/aliveOnly.luau
return function(player: Player): (boolean, string?)
	local character = player.Character
	local humanoid = character and character:FindFirstChildOfClass("Humanoid")
	if humanoid and humanoid.Health > 0 then
		return true
	end
	return false, "dead"
end
```

```lua
return RoGrid.fn({
	guards = { aliveOnly },
	limit = 1,
}, function(player: Player, input: Input): Output
```

A file named `_guard.luau` applies to every function in its folder and below.
Everything under `admin/` is admin-only because of where it lives.

```lua
-- src/server/functions/admin/_guard.luau
local Admins = require("../../systems/Admins")

return function(player: Player): (boolean, string?)
	return Admins.has(player), "forbidden"
end
```

Guards receive `(player, input)`. The input has been validated by the time a
guard sees it.

## Order of checks for one call

1. Rate limit for this player and function.
2. Input validation.
3. Folder guards, outermost folder first.
4. The function's own guards, in list order.
5. The handler.

The first check to refuse ends the call. Cheap checks run first so that spam
costs the server as little as possible.

## Starting the server

```lua
-- src/server/main.server.luau
local RoGrid = require("@game/ReplicatedStorage/Packages/rogrid")

RoGrid.start()
```

`start` opens the boundary. It is explicit so that you decide when, for example
after your data systems are ready. It accepts optional hooks:

| Hook          | Called when                                                  |
| ------------- | ------------------------------------------------------------ |
| `onViolation` | A client sent input that failed validation                   |
| `onError`     | A handler raised an error                                    |

The client needs no entry point. Requiring `api` or `events` is enough.

## Runtime guarantees

- A call made before the server has started waits for it, up to the timeout.
- An event fired before the client has a listener is queued and delivered to the
  first listener.
- Every request times out. Nothing waits forever.
- A handler error never reaches the client as text. The client sees `internal`.

## Security defaults

- Every input is validated before any guard or handler runs.
- Tables are strict. Unknown fields are rejected.
- `number` rejects NaN and infinities.
- Strings, arrays, and maps have a maximum length.
- Every function is rate limited per player even when `limit` is not set.
- In Studio a violation prints the exact field that failed. In a live game it is
  dropped silently and reported to `onViolation`.

Proposed defaults, all tunable: ten calls per second, ten second timeout, and a
length cap of one thousand twenty four for strings and collections.

Structural checks are generated. Business rules such as ranges, ownership, and
distance belong in guards or the handler.

## Types the generator understands

Input and event payload types may use:

- `string`, `number`, `boolean`, `buffer`
- String literals and unions of them
- Optionals with `?`
- Tables with named fields, arrays `{T}`, and maps `{[string]: T}`
- Unions of any of the above
- Roblox datatypes such as `Vector3`, `CFrame`, `Color3`, `EnumItem`
- Instances, checked with `IsA`, such as `Player`, `BasePart`, `Model`
- Type aliases defined in the same file, or imported through a relative
  string require
- `unknown`, as an explicit opt-out. The handler must narrow it

Anything else is a generation error with the file, the line, and the reason:
functions, metatables, `typeof`, generics with parameters, intersections, and
`any`. `any` is rejected because it would switch validation off without saying so.

```
error: src/server/functions/shop/buyItem.luau:9
  field `discount` has type `typeof(Shop.discounts)`, which cannot be checked at runtime

  Inputs can use string, number, boolean, buffer, tables, arrays, maps,
  optionals, unions, string literals, and Roblox types such as Vector3 or Player.
```

Output types are copied into the client module but never validated.

## Generated output

```
.rogrid/                  gitignored, like .next
  client/                 mapped to ReplicatedStorage.RoGrid
    api.luau              typed stubs for every function
    events.luau           client view of events
  server/                 mapped to ServerScriptService.RoGrid
    functions.luau        registry, validators, and guard chains
    events.luau           server view of events
```

The generated code is plain, readable Luau. For `buyItem` it contains the
validator you would otherwise write by hand:

```lua
local function checkShopBuyItem(input: any): (boolean, string?)
	if type(input) ~= "table" then
		return false, "input: expected table"
	end
	if type(input.itemId) ~= "string" or #input.itemId > 1024 then
		return false, "input.itemId: expected string"
	end
	local quantity = input.quantity
	if quantity ~= nil and (type(quantity) ~= "number" or quantity ~= quantity) then
		return false, "input.quantity: expected number"
	end
	for key in input do
		if key ~= "itemId" and key ~= "quantity" then
			return false, `input.{key}: unknown field`
		end
	end
	return true
end
```

and the typed stub for the client:

```lua
export type ShopBuyItemInput = { itemId: string, quantity: number? }
export type ShopBuyItemOutput = { balance: number }

return {
	shop = {
		buyItem = client.call("shop/buyItem") :: (input: ShopBuyItemInput) -> ShopBuyItemOutput,
	},
}
```

Named aliases are emitted once and referenced, so recursive types work.

The transport is an implementation detail: a small fixed set of remotes,
multiplexed by function name, with call ids for timeouts. Because the generator
knows every type, the wire format can later move to packed buffers without any
change to user code.

## Commands

```
rogrid dev                         generate, watch, and run rojo serve
rogrid build -o game.rbxl          generate, then rojo build
rogrid check                       generate and report errors, for CI
rogrid add function shop/buyItem   scaffold a function file
```

## Project layout

```
src/
  server/
    main.server.luau
    functions/            everything the client can call
      shop/
        buyItem.luau
        getCatalog.luau
      combat/
        aim.luau
      admin/
        _guard.luau
        kick.luau
    systems/              yours
    guards/               yours
  client/                 yours
  shared/
    events.luau           everything the server can tell the client
.rogrid/                  generated
```

The two conventional paths can be moved in an optional `rogrid.toml`. Everything
marked yours has no rules.

## Versions

Generated code targets a specific runtime API, so the CLI and the library are
coupled. The generated code carries a runtime API number. The library refuses a
mismatch with a message that says which side to update. `rogrid init` already
pins the CLI in the project's tool manifest and the library in the package
manifest, so a team runs matching versions.

## Requires

Roblox supports `./`, `../`, `@self`, and `@game` in string requires. Custom
aliases are not supported in the engine yet, so RoGrid uses `@game` for anything
outside the current folder. When aliases ship, the template will add `@api` and
`@events`.

## Out of scope for the first version

- Replicated state. It is the natural third primitive and the design leaves room
  for it.
- Packed buffer serialization.
- Module loading, services, or any lifecycle. Roblox already runs scripts.
- UI routing.
- Type imports through `@game` paths or instance-path requires.

## Open questions

- Whether Luau narrows `call.ok` on a boolean-tagged union in both type solvers.
  If not, `RoGrid.try` uses a string tag like the output example.
- Whether returning a table literal against a tagged-union return type checks
  cleanly in the old solver, or whether the docs should recommend the new one.
- How the generator finds the DataModel path of `functions/`. The plan is to
  read the Rojo project file, which the template controls.
- Whether folder guards should also be able to see which function is being
  called, for logging.

## Build order

1. This page.
2. The runtime library: transport, rate limiter, guard chain, event queueing,
   `fn`, `start`, `try`.
3. The generator and the `dev`, `build`, and `check` commands, using the
   `full_moon` parser.
4. The template: one function, one event, and the `.rogrid` mappings.
5. Publish both, scaffold a project, and press Play in Studio.
