//! Shared internal settings, compiled into the CLI and maintainer tools.
// Each program uses its own subset of these settings.
#![allow(dead_code)]

// Applied after each manager normalizes dashes/underscores.
// These rules follow the registries, not a RoGrid naming policy.

// https://github.com/pesde-pkg/pesde/blob/main/src/names.rs
pub const PESDE_PACKAGE_NAME: &str =
    r"\A(?=.{1,32}\z)(?![0-9]+\z)[a-z0-9](?:[a-z0-9_]*[a-z0-9])?\z";
pub const PESDE_PACKAGE_NAME_ERROR: &str = "Pesde package names must be 1–32 characters long, \
    use only lowercase letters, digits and underscores, not start or end with an underscore \
    or dash, and not contain only digits";

// https://github.com/UpliftGames/wally/blob/main/src/package_name.rs
pub const WALLY_PACKAGE_NAME: &str = r"\A[a-z0-9-]{1,64}\z";
pub const WALLY_PACKAGE_NAME_ERROR: &str = "Wally package names must be 1–64 characters long, \
    using only lowercase letters, digits and dashes";

/// Milliseconds without relevant file changes before dev regenerates code.
pub const DEV_DEBOUNCE_MS: u64 = 100;

/// Milliseconds between checks for Rojo exit while dev is idle.
pub const DEV_ROJO_POLL_MS: u64 = 250;

/// Stack space for the recursive Luau parser (16 MiB).
pub const PARSER_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Roblox drops larger UnreliableRemoteEvent payloads after engine encoding.
/// Reference: https://create.roblox.com/docs/reference/engine/classes/UnreliableRemoteEvent
/// This records an engine constraint; RoGrid does not estimate or enforce wire sizes.
pub const ROBLOX_UNRELIABLE_PAYLOAD_BYTES: usize = 1000;

/// Approximate Roblox client-to-server rate, shared among remotes of the same type.
/// Reference: https://create.roblox.com/docs/reference/engine/classes/UnreliableRemoteEvent
/// Roblox performs throttling. This is not a RoGrid traffic policy.
pub const ROBLOX_REMOTE_EVENTS_PER_SECOND_PER_CLIENT: usize = 500;

/// Seconds allowed to establish a registry connection during release checks.
pub const REGISTRY_CONNECT_TIMEOUT_SECS: u64 = 30;

/// Total seconds allowed per registry download attempt, including connection time.
pub const REGISTRY_DOWNLOAD_TIMEOUT_SECS: u64 = 90;

/// Additional attempts after a temporary registry download failure.
pub const REGISTRY_DOWNLOAD_RETRIES: u64 = 2;

/// Total checks per registry after uploading a release, including the first check.
pub const REGISTRY_VERIFY_ATTEMPTS: usize = 12;

/// Seconds between checks for a newly uploaded release.
pub const REGISTRY_VERIFY_INTERVAL_SECS: u64 = 5;

/// Maximum registry download size during release checks (10 MiB).
pub const REGISTRY_DOWNLOAD_MAX_BYTES: u64 = 10 * 1024 * 1024;

/// Maximum size of each decompressed file in a release package (10 MiB).
pub const ARCHIVE_FILE_MAX_BYTES: u64 = 10 * 1024 * 1024;
