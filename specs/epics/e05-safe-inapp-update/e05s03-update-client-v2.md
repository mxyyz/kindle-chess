# e05s03 — App update client v2: bundle lookup, preflight, staged download

**type:** feat | **risk:** P0 | **bcps:** 8 | **delta:** MODIFIED

## Context
The current updater (update.rs) downloads only the release binary to `<exe>.new`. e05s03 extends it to find the bundle asset, preflight device sanity (arch/dirs/writable/free space), stream-download the bundle to `<exe>.bundle.new`, verify the sidecar sha256, extract to a staging dir, and verify the per-file MANIFEST — never touching the running install.

## Requirements

#### MODIFIED: github.rs finds the bundle asset
**Before:** check_for_update looks for `kindle-chess-armv7-musl{,.sha256}` only.
**After:** looks for `update.tar.gz{,.sha256}` first, falls back to legacy pair; UpdateInfo gains bundle_url, bundle_sha256_url, bundle_size.

#### ADDED: preflight sanity checks
Arch (asset name contains armv7 AND runtime uname -m == armv7l), dirs exist+writable, free space via `df -k ROOT_DIR` >= 2x bundle_size + 10% margin. Returns Result<Preflight, ChessError> with human-readable reason.

#### MODIFIED: apply_update v2 stages the bundle
**Before:** streams binary to `<exe>.new`, verifies sha256, sets exec bit.
**After:** preflight → stream bundle to `<exe>.bundle.new` hashing → verify sidecar → extract to `<exe>.bundle.staging/` (shell out to tar) → verify MANIFEST per file (pure-Rust sha2) → on any error remove .new/.staging, return Err. Original untouched.

#### ADDED: unit tests for parsers
parse_df_k, manifest verify (ok/mismatch/missing), preflight allow/deny, wiremock for GitHub bundle-asset lookup.

## Steps
1. github.rs bundle constants + UpdateInfo fields + wiremock tests → verify: `cargo test --lib api::github`
2. update.rs preflight() + parser unit tests → verify: `cargo test --lib api::update`
3. apply_update v2 (staged bundle flow) → verify: `cargo build && cargo test`
4. fmt + clippy clean → verify: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`

## Verification Script
1. `cd crate && cargo test --lib api::github` — bundle lookup tests pass
2. `cd crate && cargo test --lib api::update` — preflight + parser tests pass
3. `cd crate && cargo clippy --all-targets -- -D warnings` — clean

## Out of scope
- Asset embedding in the binary (user decision, spike documented)
- In-place binary replacement while running (staged approach only)

## Risks
- df -k output format varies across devices → parser handles 1K-blocks/fsp lines, tested
- tar not available on some device → preflight checks; spike proved busybox tar works on k2
- No new runtime deps — tar invoked via std::process, sha2 already a dep
