# Releasing RoGrid

Releases are managed by `JustSammyyy` through GitHub Actions. The CLI and runtime
have separate versions; the release tools update their manifests and pins together.

## Publish a release

1. Run **Actions > Prepare release** on `main`. Choose a CLI version bump and
   a runtime bump. Use `keep` for the runtime only when `packages/rogrid/` is
   unchanged, including its packaged docs.
2. Review the generated PR and edit `releases/vX.Y.Z.md`. For runtime changes,
   check the playground in Studio. Merge once all CI checks pass.
3. Run **Actions > Publish release** on `main` and enter the merged PR number.

Publishing repeats the checks and builds from that PR's merge commit. The runtime
is published to Pesde and Wally first. After both packages pass installation checks,
the workflow creates the CLI tag and GitHub release.

## Retry a failed release

Use **Re-run failed jobs** on the original run. If its artifacts have expired,
run **Publish release** again with the same PR number.

A release can reach one registry before failing on the other. Retries verify and
skip matching packages already published; conflicting contents stop the release.
A completed CLI release is left unchanged. Fix incorrect published code in a new
release rather than reusing its version.

## Prepare versions locally

From a clean repository root:

```sh
cargo xtask release prepare --cli minor --runtime patch
```

For a CLI-only patch:

```sh
cargo xtask release prepare --cli patch --runtime keep
```

These commands update files without committing or publishing. Run
`cargo xtask release --help` for the other checks and packaging commands.
