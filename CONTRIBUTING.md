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

For changes to Wally integration or runtime packaging, install Wally 0.3.2
and Rojo 7.7.0 on PATH and run the opt-in checks:

```sh
cargo test -p rogrid --test wally_tools -- --ignored
```

These exercise real local-framework setup, a Rojo build, exact dependency
parsing, and package contents without publishing. Check startup in Studio
separately when changing runtime behavior or package layout.

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

Document behavior that exists and distinguish source-only features from
published releases. Keep [project status](packages/rogrid/docs/status.md)
current when releasing. Do not use em dashes or promise that breaking changes
end at a particular version.

## Releases

Maintainers use **Prepare release**, review and merge its PR after CI passes,
then run **Publish release** with that PR number. Publishing repeats checks on
the exact merged commit before uploading anything.

See the [release guide](.github/RELEASING.md) for publishing, local version
updates, and retrying failed releases.
