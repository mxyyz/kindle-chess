// Build/version metadata. `VERSION` is sourced from Cargo.toml (the single
// source of truth); `GIT_SHA` and `BUILD_TIMESTAMP` are baked in by build.rs.

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GIT_SHA: &str = env!("GIT_SHA");
pub const BUILD_TIMESTAMP: &str = env!("BUILD_TIMESTAMP");

/// Human-readable composite, e.g. `0.1.0 (26a264a, 2026-05-07T12:34:56Z)`.
pub fn full() -> String {
    format!("{} ({}, {})", VERSION, GIT_SHA, BUILD_TIMESTAMP)
}

/// Parsed semver of the running binary. Used for comparisons against the
/// latest GitHub release tag.
pub fn current() -> semver::Version {
    semver::Version::parse(VERSION).expect("CARGO_PKG_VERSION must be valid semver")
}
