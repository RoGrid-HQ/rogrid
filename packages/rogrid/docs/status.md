---
sidebar_position: 8
---

# Project status

RoGrid is unfinished and under active development. Breaking changes can occur
in any release, including changes to the API, generated code, project layout,
and configuration. Review release notes and test your game when upgrading.

## Release availability

| Component | Distribution |
| --- | --- |
| CLI | [GitHub releases](https://github.com/RoGrid-HQ/rogrid/releases), including installation through Rokit. Supports Pesde and Wally projects. |
| Luau runtime on Pesde | Published as [rogrid/rogrid](https://pesde.dev/packages/rogrid/rogrid). |
| Luau runtime on Wally | Published as `rogrid-hq/rogrid` in the [Wally index](https://github.com/UpliftGames/wally-index). |

The starter pins a matching runtime version. Update the CLI, runtime, and
generated code together when a change requires it. Generated code checks the
runtime protocol and generated revisions during startup.

## Available functionality

- Project initialization with package and tool manifests.
- Typed client-to-server and server-to-client event callers.
- Payloads with Roblox values, Instance references, nested tables, optionals,
  unions, and type aliases.
- Generated payload validation.
- Configurable event folders resolved through Rojo sourcemaps.
- A development watcher and generation before place builds.
- Local framework development without publishing packages.

## Current limitations

Events use ordinary RemoteEvents and have no request/reply API, custom serialization, automatic
retries, or client readiness protocol. See the [API reference](./api.md) for
declaration rules and runtime limits.

Generated interfaces require the CLI and Rojo. Installing the runtime alone
does not configure a game. Studio Play must be restarted after changes;
there is no hot reload of running modules.

Wally with Pesde as tool manager is not supported by the CLI. Other managers
are not implemented. See [package managers](./package-managers.md) for current
combinations.
