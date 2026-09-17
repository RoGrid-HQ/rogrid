# RoGrid syntax review

Four commands, one per way of talking across the network. Only `RoGrid.request` is
built so far, without `limit` and `timeout`. This page exists so you can judge the syntax. It supersedes the `RoGrid.fn`
naming in the design page, which gets updated once you approve.

|                      | Waits for an answer | Does not wait     |
| -------------------- | ------------------- | ----------------- |
| **Client to server** | `RoGrid.request`    | `RoGrid.message`  |
| **Server to client** | `RoGrid.query`      | `RoGrid.event`    |

Examples leave out these lines:

```lua
local RoGrid = require("@game/ReplicatedStorage/Packages/rogrid")
local api = require("@game/ReplicatedStorage/RoGrid/api")            -- client: call the server
local events = require("@game/ReplicatedStorage/Shared/events")      -- both sides
local queries = require("@game/ServerScriptService/RoGrid/queries")  -- server: ask a client
```

## 1. `RoGrid.request`

The client asks, the server answers, the client waits. One file per request in
`src/server/functions/`. The path is the name. The input is validated from its
type before the handler runs.

**With input and output.** Expected failures live in the output type.

```lua
-- src/server/functions/shop/buyItem.luau
type Input = { itemId: string, quantity: number? }
type Output =
	{ status: "ok", balance: number }
	| { status: "failed", reason: "unknown-item" | "insufficient-funds" }

return RoGrid.request(function(player: Player, input: Input): Output
	local item = Shop.find(input.itemId)
	if not item then
		return { status = "failed", reason = "unknown-item" }
	end
	local balance = Wallet.spend(player, item.price * (input.quantity or 1))
	if not balance then
		return { status = "failed", reason = "insufficient-funds" }
	end
	return { status = "ok", balance = balance }
end)
```

```lua
-- client
local result = api.shop.buyItem({ itemId = "sword", quantity = 2 })
if result.status == "ok" then
	coinsLabel.Text = tostring(result.balance)
else
	toast(result.reason)
end
```

**Without input.**

```lua
-- src/server/functions/shop/getCatalog.luau
type Item = { id: string, name: string, price: number }

return RoGrid.request(function(player: Player): { Item }
	return Shop.catalogFor(player)
end)
```

```lua
-- client
for _, item in api.shop.getCatalog() do
	addRow(item.name, item.price)
end
```

**With options and no return value.** The client still waits until the server is done.

```lua
-- src/server/functions/settings/save.luau
type Input = { musicVolume: number, showHints: boolean }

return RoGrid.request({
	limit = 1, -- calls per second, per player
	timeout = 5,
}, function(player: Player, input: Input)
	Profiles.update(player, { settings = input })
end)
```

```lua
-- client: RoGrid.try turns a failed call into a value instead of an error
local call = RoGrid.try(api.settings.save, { musicVolume = 0.5, showHints = true })
if call.ok then
	menu:close()
else
	toast("Could not save: " .. call.code) -- "rate-limited", "timeout", "internal", ...
end
```

## 2. `RoGrid.message`

The client tells the server and moves on. Same folder and same validation as a
request. The client call returns immediately. Returning a value is an error.

**Basic.**

```lua
-- src/server/functions/emotes/play.luau
type Input = { emote: "wave" | "dance" | "cheer" }

return RoGrid.message(function(player: Player, input: Input)
	Emotes.play(player, input.emote)
end)
```

```lua
-- client
api.emotes.play({ emote = "wave" })
```

**Unreliable, for data sent every frame.** A lost packet does not matter because
the next one replaces it.

```lua
-- src/server/functions/combat/aim.luau
type Input = { direction: Vector3 }

return RoGrid.message({
	unreliable = true,
	limit = 60,
}, function(player: Player, input: Input)
	Aim.set(player, input.direction)
end)
```

```lua
-- client
RunService.RenderStepped:Connect(function()
	api.combat.aim({ direction = camera.CFrame.LookVector })
end)
```

**With guards.** A guard is a plain function, `(player, input) -> (boolean, string?)`,
that runs before the handler and can refuse the call.

```lua
-- src/server/functions/world/pickUp.luau
type Input = { item: BasePart }

return RoGrid.message({
	guards = { aliveOnly, cooldown(0.5) },
}, function(player: Player, input: Input)
	if Distance.within(player, input.item, 12) then
		Items.pickUp(player, input.item)
	end
end)
```

```lua
-- client
api.world.pickUp({ item = hoveredPart })
```

## 3. `RoGrid.event`

The server tells clients. All events are declared in one shared file that both
sides require. No validation, because the client trusts the server. No code is
generated for events.

```lua
-- src/shared/events.luau
return RoGrid.events({
	coinsChanged = RoGrid.event<<{ coins: number }>>(),
	roundStarted = RoGrid.event<<{ map: string, endsAt: number }>>(),
	hit = RoGrid.event<<{ position: Vector3, damage: number }>>({ unreliable = true }),
})
```

If `<<T>>` turns out not to be usable everywhere yet, the fallback is
`RoGrid.event() :: RoGrid.Event<{ coins: number }>`.

**To one player.**

```lua
-- server
events.coinsChanged:fire(player, { coins = Wallet.get(player) })

-- client
events.coinsChanged:on(function(payload)
	coinsLabel.Text = tostring(payload.coins)
end)
```

**To everyone.**

```lua
-- server
events.roundStarted:fireAll({ map = "Crossroads", endsAt = os.time() + 300 })

-- client
events.roundStarted:on(function(payload)
	banner:show(payload.map)
	timer:countTo(payload.endsAt)
end)
```

**To everyone but one, and how to stop listening.**

```lua
-- server: the attacker already drew the effect locally
events.hit:fireExcept(attacker, { position = hitPosition, damage = 25 })

-- client
local disconnect = events.hit:on(function(payload)
	Effects.spark(payload.position)
end)

disconnect()
```

`fireList(players, payload)` sends to a chosen list. An event fired before the
client has a listener is queued for the first listener.

## 4. `RoGrid.query`

The server asks one client and waits. One file per query in `src/client/queries/`.
This is the mirror image of a request, so the trust flips: the **reply** is
validated on the server from the output type. The call returns `nil` when the
client does not answer in time, answers with the wrong shape, or leaves.

A client can lie. Treat an answer as a hint, never as truth. This command was not
planned for the first version. It is here so you can judge the full set.

**Without input.**

```lua
-- src/client/queries/camera.luau
type Output = { position: Vector3, look: Vector3 }

return RoGrid.query(function(): Output
	local cframe = workspace.CurrentCamera.CFrame
	return { position = cframe.Position, look = cframe.LookVector }
end)
```

```lua
-- server
local camera = queries.camera(player)
if not camera then
	return
end
Spectate.follow(player, camera.position, camera.look)
```

**With input and a long timeout.**

```lua
-- src/client/queries/confirmTrade.luau
type Input = { from: string, items: { string } }
type Output = { accepted: boolean }

return RoGrid.query({ timeout = 30 }, function(input: Input): Output
	return { accepted = TradeDialog.ask(input.from, input.items) }
end)
```

```lua
-- server
local answer = queries.confirmTrade(target, { from = player.Name, items = offer })
if answer and answer.accepted then
	Trades.complete(player, target, offer)
end
```

**Used as a hint.**

```lua
-- src/client/queries/device.luau
type Output = { platform: "pc" | "mobile" | "console", screen: Vector2 }

return RoGrid.query(function(): Output
	return { platform = Device.platform(), screen = workspace.CurrentCamera.ViewportSize }
end)
```

```lua
-- server
local device = queries.device(player)
local quality = if device and device.platform == "mobile" then "low" else "high"
```

## Options per command

| Option       | request | message | event | query | Meaning                              |
| ------------ | :-----: | :-----: | :---: | :---: | ------------------------------------ |
| `guards`     |   yes   |   yes   |       |       | Functions that can refuse the call   |
| `limit`      |   yes   |   yes   |       |       | Calls per second, per player         |
| `timeout`    |   yes   |         |       |  yes  | Seconds before the caller gives up   |
| `unreliable` |         |   yes   |  yes  |       | Faster delivery that may drop        |

An option that is not valid for a command is a type error in the editor.

## What to decide while reading

- The four names: `request`, `message`, `event`, `query`.
- One file per request, message, and query, with the path as the name.
- All events in one file, declared with `<<T>>`.
- Whether `query` belongs in the first version at all.
