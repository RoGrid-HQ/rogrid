# Contributing to RoGrid

RoGrid is unfinished and under active development. Keep changes focused,
cover new behavior with tests, and update the docs with the implementation.
Breaking changes can occur in any release.

## Repository layout

| Directory | Purpose |
| --- | --- |
| `crates/rogrid/` | Rust CLI, initialization, and event generation. |
| `packages/rogrid/` | Luau runtime and package metadata. |
| `packages/rogrid/docs/` | User guides and reference documentation. |
| `templates/places/default/` | Starter game embedded in the CLI. |
| `playground/` | Game that uses the local CLI and runtime source. |
| `xtask/` | Maintainer commands for version updates and release verification. |

## Local development

Install a stable Rust toolchain, Rokit, and Roblox Studio. From the repository
root, start the [playground](playground/README.md):

```sh
cd playground
rokit install
rojo plugin install
cargo run --manifest-path ../Cargo.toml -p rogrid -- dev
```

The playground maps the runtime source directly through Rojo. It does not
need package publication or installation. Restart Studio Play after Luau
edits and restart Cargo after Rust edits.

To exercise initialization, use
[`init --local-framework`](packages/rogrid/docs/cli.md#local-framework). This
runs package and tool installation while loading the runtime from source.

## Checks

From the repository root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
lune run test
```

CI runs formatting, Clippy, tests, and a release build on Linux, macOS, and
Windows. Fix warnings as well as errors.

### Writing tests

Follow the [Rust book's test organization](https://doc.rust-lang.org/book/ch11-03-test-organization.html):
put focused unit tests in a `#[cfg(test)] mod tests` beside the code they
exercise. Keep tests that run the CLI in `crates/rogrid/tests/`.

Initialization integration tests cover the supported manager combinations
with both registry and local framework setups, validation, install order,
and failure recovery. They run the real CLI against small native stand-ins
for external tools, using temporary directories and an isolated home and
PATH. They do not download registry packages.

The suite also runs `dev` against real filesystem notifications and a stand-in
Rojo process, checking regeneration, invalid-source recovery, and shutdown.
Interactive `init` and Ctrl+C tests use a pseudoterminal through the dev-only
`portable-pty` dependency. Release tests use inert installers and registry
responses in isolated subprocesses; they never upload packages. The registry
verification timeout test takes about 55 seconds.

For changes to Wally integration or runtime packaging, install Wally 0.3.2
and Rojo 7.7.0 on PATH and run the opt-in checks:

```sh
cargo test -p rogrid --test wally_tools -- --ignored
```

These exercise real local-framework setup, a Rojo build, exact dependency
parsing, and package contents without publishing. Check startup in Studio
separately when changing runtime behavior or package layout.

### Payload tests and types

Repository `rokit.toml` pins Lune 0.10.5. `lune run test` runs the real shared
validator against valid and invalid payloads, including native Roblox values
implemented by Lune. CI runs this suite in the package checks job.
It also executes the real runtime with small engine stand-ins to check
lifecycle guards, validation, diagnostics, and handler-error recovery. These
stand-ins do not simulate Roblox networking or replication.

CI also checks generated payload annotations and descriptors with luau-lsp
1.69.0. To run that check locally, put luau-lsp on PATH, set
`ROGRID_ROBLOX_DEFINITIONS` to its Roblox `globalTypes.d.luau` definitions file,
and run `cargo test -p rogrid payloads_pass_luau_typechecking -- --ignored`.

With Wally, Pesde, Rojo, and the same typechecker setup installed,
`cargo test -p rogrid --test project_tools -- --ignored` initializes real local
framework projects with both managers, builds them, and typechecks complete
generated modules. To execute fresh generator output under Lune, including
protocol and revision mismatch checks, run:

```sh
cargo test -p rogrid generated_startup_and_compatibility_guards_execute -- --ignored
```

Both checks run in the package CI job.

For real Roblox transport and runtime dispatch, use Studio and Rojo's
`run-in-roblox` 0.3.0:

```sh
rojo build packages/rogrid/tests/studio.project.json --output target/payload-tests.rbxlx
run-in-roblox --place target/payload-tests.rbxlx --script packages/rogrid/tests/studio-run.luau
```

The fixture tests both network directions, including DateTime (absent in Lune),
Font and shared Model references. It also checks that invalid messages never
reach the runtime handler and a burst of 100 valid messages is fully delivered.
It opens a temporary Studio test session and exits.

For public startup and multiplayer coverage, set `ROGRID_STUDIO_RUNNER` to the
absolute path of `run-in-roblox`, then run:

```sh
cargo test -p rogrid --test studio -- --ignored
```

This test uses real Wally initialization and generated callers in two-client
Studio sessions, through direct and linked package imports. It checks targeted
delivery, broadcasts, sender identity, invalid client-bound payloads, and
handler-error recovery. Studio tests run separately from normal CI.

The network type model lives in `codegen/types.rs`; built-in runtime checks
live in `packages/rogrid/src/validate.luau`. Add/remove a leaf in those two places
and adjust `tests/native.luau`; the Lune suite checks this list stays in sync.
Class and enum names are generated snapshots from Lune's Roblox reflection
database. Refresh them with `lune run roblox-types` when updating test tooling.
Alias resolution and rendering use the same model as validation descriptors.
Keep this model specific to network payloads.

## Adding a manager

Manager-specific descriptors, naming rules, and manifest templates live
together in `crates/rogrid/src/tools/`. Add a module and template, then
register the descriptor in `tools/mod.rs`. Keep the common starter under
`templates/places/default/` independent of the selected manager.

Add focused unit tests and extend the CLI integration matrix for the new
supported combinations. Update the [package manager guide](packages/rogrid/docs/package-managers.md)
to match actual install behavior and release availability.

## Documentation

User documentation lives in `packages/rogrid/docs/`. Update it alongside
implementation changes. The package includes these Markdown files for Pesde's
documentation renderer, and they can also be read in the repository.

Edit the relevant `.md` file using ordinary Markdown. New pages should have a
level-one title and `sidebar_position` frontmatter for ordering. Use relative
links such as `events.md`, including anchors where useful, and update the
README's documentation list when adding a guide.

Document the behavior of the code in this checkout. Update feature docs with
the implementation and describe supported functionality in the present tense.
User-facing docs ship with the release; keep release preparation notes and
version bookkeeping in maintainer documentation. Keep
[project status](packages/rogrid/docs/status.md) current. Do not use em dashes
or promise that breaking changes end at a particular version.

## Releases

Maintainers use **Prepare release**, review and merge its PR after CI passes,
then run **Publish release** with that PR number. Publishing repeats checks on
the exact merged commit before uploading anything.

See the [release guide](.github/RELEASING.md) for publishing, local version
updates, and retrying failed releases.
