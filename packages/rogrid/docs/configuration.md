---
sidebar_position: 3
---

# Configuration

Run RoGrid from the game folder containing `default.project.json`. The starter
is ready to use without a `rogrid.toml` file.

## Event folders

By default, RoGrid reads `src/server/events` and `src/client/events`. To change
them, create `rogrid.toml` in the project root:

```toml
[events]
server = ["src/server/events", "src/server/combat/events"]
client = ["src/client/events"]
```

Each list replaces that side's defaults. An omitted side keeps its default;
an empty list, such as `client = []`, disables discovery for that side.

Folders must exist inside the project. Use project-relative paths without a
leading `./` or any `..` components, and keep them outside `.git` and `.rogrid`.
Redundant `.` components within a path are accepted: `src/./events` resolves
to the same folder as `src/events`. Unknown configuration keys and duplicate
folders are errors.

Only direct `.luau` and `.lua` module files are discovered. List subfolders
separately if needed. Module names must be unique across all folders on each
side, so two server folders cannot both contain `Lobby.luau`.

Folder names do not appear in callers. A `Lobby.luau` module still generates
`Server.Lobby` regardless of which server folder contains it.

## Rojo mapping

Map event folders into the game in `default.project.json`. The starter maps
all of `src/server` and `src/client`, so folders beneath them are already
included.

RoGrid uses a fresh Rojo sourcemap to find each receiver. Each file must map
to exactly one ModuleScript in a supported location:

| Receiver | Supported Roblox locations |
| --- | --- |
| Server | `ServerScriptService` or `ServerStorage` |
| Client | `StarterPlayer.StarterPlayerScripts` or `ReplicatedStorage` |

Client modules under `StarterPlayerScripts` are located in the local player's
`PlayerScripts` at runtime. RoGrid follows the mapped instance names, including
renamed folders and modules. Streamed `Workspace` content and
`StarterCharacterScripts` are not supported receiver locations.

## Generated code

RoGrid writes `.rogrid/generated/` and ensures these Rojo mappings exist:

| Source directory | Roblox location |
| --- | --- |
| `.rogrid/generated/shared` | `ReplicatedStorage.RoGridGenerated` |
| `.rogrid/generated/server` | `ServerScriptService.RoGridGenerated` |
| `.rogrid/generated/client` | `StarterPlayer.StarterPlayerScripts.RoGridGenerated` |

The corresponding service nodes must exist in the Rojo project. RoGrid adds
missing generated mappings and refuses conflicting mappings. Other project
settings are preserved.

The runtime must be available as `ReplicatedStorage.Packages.rogrid`. The
starter configures this for the selected [package manager](./package-managers.md).

Do not edit or commit `.rogrid/`. It can be regenerated from source. The
`.rogrid/events.json` cache records the previous successful event inventory;
without it, all current events are reported as added.

## Editor settings

The starter includes VS Code settings for the Luau Language Server. For an
existing editor setup, merge these settings into `.vscode/settings.json`:

```json
{
  "luau-lsp.platform.type": "roblox",
  "luau-lsp.sourcemap.enabled": true,
  "luau-lsp.sourcemap.autogenerate": true,
  "luau-lsp.sourcemap.rojoProjectFile": "default.project.json",
  "luau-lsp.diagnostics.strictDatamodelTypes": true
}
```

Use `--!strict` in Luau modules for type diagnostics. `rogrid dev` preserves
existing editor settings and only creates defaults when the settings file is
absent. `rogrid init --force` overwrites starter files, including editor settings.

## Watching changes

`rogrid dev` watches project Luau sources, event folders, `rogrid.toml`, and
Rojo project files. This includes shared type modules and the creation or
removal of imports. Changes under `.rogrid` and `.git` are ignored.
Imports outside the project root are unsupported.

Invalid configuration or source blocks generation and marks generated startup
unusable until generation succeeds. Fix the error and save again, then restart
Studio Play. RoGrid does not hot-reload a running game.
