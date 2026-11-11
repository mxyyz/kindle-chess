# e05s05 — UpdateScreen UX + i18n for the bundle flow

**type:** feat | **risk:** P1 | **bcps:** 5 | **delta:** MODIFIED

## Context
UpdateScreen currently shows check → available → downloading → applied/failed for the binary-only flow. e05s05 extends it for the bundle: preflight failure reasons, bundle size/contents, download progress, and the relaunch hint. All new strings EN/DE.

## Requirements

#### MODIFIED: UpdateScreen states extended
**Before:** Checking / UpToDate / Available / Downloading / Applied / Failed.
**After:** + PreflightFailed(reason) showing arch/dir/space reason with retry; Available shows bundle size + contents; Downloading shows progress; Applied → "relaunch to finish" hint.

#### ADDED: EN/DE i18n keys
New fluent keys (update-preflight-failed, update-bundle-size, update-downloading, update-applied-relaunch, ...) in both en.ftl and de.ftl.

## Steps
1. UpdateScreen state machine + rendering → verify: `cargo build && cargo test --lib ui`
2. i18n keys en/de → verify: all en.ftl update-* keys present in de.ftl
3. fmt + clippy → verify: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`

## Verification Script
1. `cd crate && cargo test --lib ui` — screen tests pass
2. `grep -c 'update-' crate/i18n/en.ftl crate/i18n/de.ftl` — equal counts

## Out of scope
- Changing the check/apply event flow (AppEvent variants unchanged)

## Risks
- Layout overflow with longer preflight messages → keep lines short, test at 28px font
