---
sidebar_position: 4
---

# Package managers

A package manager installs Luau libraries. A tool manager installs command-line
programs such as Rojo. RoGrid writes their manifests and runs their install
commands during `init`.

## Supported combinations

| Package manager | Supported tool managers |
| --- | --- |
| Pesde | Rokit, Pesde, `none` |
| Wally | Rokit, `none` |

Wally with Pesde as tool manager is not supported by the current CLI. Pesde's
tool setup is currently part of its package manifest; the Wally adapter does
not create that extra manifest. This is a RoGrid integration limitation, not
an inherent conflict between Wally and Pesde.

## Pesde

The Pesde setup uses the `rogrid/rogrid` package:

```sh
rogrid init my-game --package-manager pesde --tool-manager rokit
```

Rokit installs the pinned tools before `pesde install` runs. To let Pesde
provide its own tools, install [Pesde](https://docs.pesde.dev/installation)
first and select it instead:

```sh
rogrid init my-game --package-manager pesde --tool-manager pesde
```

Every Pesde starter declares the Rojo configuration helper and its Rojo
dependency. Pesde installs these even when the tool manager is `none`. When
using Rokit, both manifests specify the same Rojo version; keep them aligned
when updating. The two launchers can download separate copies of Rojo.

With Pesde tools, make sure its binary directory, `~/.pesde/bin`, is on PATH
before running `rogrid dev`. On Windows this is `%USERPROFILE%\.pesde\bin`.

## Wally

The Wally setup uses the `rogrid-hq/rogrid` package. With the RoGrid CLI and
Rokit installed, run:

```sh
rogrid init my-game --package-manager wally --tool-manager rokit
cd my-game
rojo plugin install
rogrid dev
```

Rokit installs the pinned CLI, Wally, and Rojo. Wally installs the framework,
and RoGrid generates the initial event interfaces. To develop the framework
itself, use [local framework setup](./cli.md#local-framework).

With `--tool-manager none`, Wally and Rojo must already run on PATH. The CLI
checks both before writing project files.

## Package locations

The dependency alias is always lowercase `rogrid`. Generated code loads the
framework from `ReplicatedStorage.Packages.rogrid` with either manager.

| Manager | Shared package directory | Server package directory |
| --- | --- | --- |
| Pesde | `roblox_packages/` | `roblox_server_packages/` |
| Wally | `Packages/` | `ServerPackages/` |

The starter maps these directories to `ReplicatedStorage.Packages` and
`ServerScriptService.ServerPackages`. Wally's `[place]` configuration matches
those locations for cross-realm dependencies.

Keep `pesde.lock` or `wally.lock` in version control. Downloaded package folders
are ignored. `none` skips separate tool-manager setup; it still runs the
package manager's install command.

## Project names

Use lowercase letters, digits, dashes, and underscores. The CLI normalizes the
manifest name for each manager without changing the project folder:

| Manager | Normalization | Valid manifest name |
| --- | --- | --- |
| Pesde | Dashes become underscores | 1 to 32 characters; no leading or trailing underscore; not all digits |
| Wally | Underscores become dashes | 1 to 64 characters |

The CLI validates manager-specific rules before creating files.
