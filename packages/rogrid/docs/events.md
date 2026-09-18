# Events

Declare the receiver on the side that runs it. The CLI generates the other
side's typed caller. Nothing in a server receiver's implementation is copied
into the shared client interface.

## Declarations

By default, put plain ModuleScripts directly in `src/server/events/` and
`src/client/events/`. Use names such as `Lobby.luau`, not `init.luau` or
`Lobby.server.luau`. Use valid Luau identifiers for module names.

Override the folders with an optional `rogrid.toml` at the project root:

```toml
[events]
server = ["src/server/events", "src/server/cooler_events"]
client = ["src/client/events"]
```

Lists replace defaults; omitted sides keep defaults, and an empty list disables
that side. Every listed folder must exist inside the project. Paths are relative
to the project root; parent traversal and absolute paths are not accepted.
Unknown configuration keys and duplicate folder entries are errors. Only direct
module files are loaded; subfolders are ignored unless separately listed.
Module filenames must be unique across all folders on each side. Duplicate
names report both source files. Folder names do not appear in the caller API.

Your Rojo project must map every receiver to exactly one ModuleScript. The CLI
asks Rojo for a fresh sourcemap during generation and uses the mapped instance
names, including renamed folders and modules. Server receivers must be under
ServerScriptService or ServerStorage, so their implementation stays private.
Client receivers may be under StarterPlayerScripts (resolved through the local
player's PlayerScripts at runtime) or ReplicatedStorage. Other locations, such
as streamed Workspace content or StarterCharacterScripts, are not supported.

Each module must end with one literal returned table:

```luau
--!strict
local RoGrid = require(game.ReplicatedStorage.Packages.rogrid)

return {
    setReady = RoGrid.event(function(player: Player, ready: boolean)
        player:SetAttribute("Ready", ready)
    end),
}
```

Declare every receiver directly as `RoGrid.event(function(...) ... end)`;
aliases, separately defined callbacks, computed exports, and generic handlers
are not part of the first version. Handler bodies can use ordinary Luau and
require other game modules. Keep helper modules outside the events folder.

Every argument must have an explicit annotation. A server receiver's first
argument must be `Player`, which Roblox supplies. All remaining arguments—and
all client receiver arguments—must be `string`, `boolean`, or `number`.
There are at most 16 payload arguments. Optional types, aliases, tables,
Instances, unions, and variadic arguments are rejected during generation.
`self` and names beginning `_rogrid` are reserved parameter names.

Events have no response. Omit the handler's return annotation or use `()`.

## Startup and calling

The server template starts the framework once:

```luau
--!strict
local RoGrid = require(game:GetService("ReplicatedStorage").Packages.rogrid)
RoGrid.start()
```

The client waits for its initial package before starting:

```luau
--!strict
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local Packages = ReplicatedStorage:WaitForChild("Packages")
local RoGrid = require(Packages:WaitForChild("rogrid"))
RoGrid.start()
```

RoGrid handles the remaining startup checks and follows the generated module
locations. No folder list is repeated in Luau. The starter runs its optional
`ReadyDemo` game module after startup; removing that call leaves only boot code.

Startup loads the receiver modules and connects them to their RemoteEvents.
Do not fire events at module top level while those modules are being loaded.
Fire from handlers or after `RoGrid.start` has returned.

Client callers:

```luau
local Server = require(game.ReplicatedStorage.RoGridGenerated.Server)
Server.Lobby.setReady.fire(true)
```

For a client module `Notifications.luau` exporting `show(message: string)`,
the generated server callers are:

```luau
local Client = require(game.ServerScriptService.RoGridGenerated.Client)
Client.Notifications.show.fire(player, "Hello!")
Client.Notifications.show.fireAll("Round starting!")
```

Event callers are plain functions: use a dot for `.fire(...)` and `.fireAll(...)`.
Their argument hints show only the values you supply, including the target
`player: Player` for a call to one client.

These functions return immediately after sending. A successful send is not an
acknowledgment that the handler ran. In particular, notifications sent before a
client finishes loading are not persistent state; the game must arrange when
to send its initial state. This release does not add a readiness protocol.

## Validation and failures

The generated receiver checks the exact number and types of payload arguments.
Numbers must not be NaN or infinity. Strings are limited to 4096 bytes.
Invalid inbound client messages are dropped before invoking game code.

The server also limits each player to a burst of 60 events, with tokens
refilling at 60 per second across all events. This is a general abuse limit,
not a replacement for a weapon cooldown or purchase authorization. The game
still checks permission, ownership, range, and any other business rules.

Client receivers check server payloads too, logging invalid messages rather
than running a handler with incorrect arguments. Handler errors are logged on
the receiving side, with the event name and traceback. They are not sent back
to the caller. Receivers should return promptly; RoGrid does not cancel a
handler that yields indefinitely.

## Generation and the editor

Run `rogrid dev` while editing. It reads event declarations with a Luau parser,
emits typed interfaces, and starts Rojo. Rojo must be available on PATH for
sourcemap generation, including with `--once`. The CLI never executes receiver
source on the development machine. Ctrl+C stops the watcher and its Rojo child
process.

The watcher follows configured event folders and listens for `rogrid.toml` and
`default.project.json` edits. It updates its watches when the lists change. It
also notices nested Rojo project files within watched source trees. Restart
`dev` after changing Rojo configuration files outside those trees. Bad config
or missing folders invalidates generated startup until generation succeeds.

The output is owned by RoGrid under `.rogrid/generated/`. Unchanged files are
not rewritten, and obsolete output files are removed. Invalid source marks
the generation unusable for the next startup; saving a valid declaration
regenerates it. Startup compares generated revisions to reject mismatched
client/server/caller files.

The CLI generates callers, validation, and startup code. The runtime comes
from `ReplicatedStorage.Packages.rogrid`, installed through pesde in normal
projects. Generated code checks the package protocol before using its internal
runtime API. For local development, the repository playground maps this same
package location directly to `packages/rogrid/src`; no publication is needed.

Use the Luau Language Server VS Code extension, `--!strict` in game modules,
Rojo sourcemaps, and `luau-lsp.diagnostics.strictDatamodelTypes = true`.
The starter configures these. Existing editor settings are preserved.

Restart Studio Play after changes. Runtime module caches are not hot-reloaded.
Run `rogrid dev --once` before `rojo build`; runtime cannot read erased Luau
annotations to discover a source edit that was never generated.

No RemoteFunctions, promises, automatic retries, custom serialization, or
UnreliableRemoteEvents are included in this version.
