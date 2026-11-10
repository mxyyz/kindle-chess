# e05s04 — chess_app.sh trampoline v2: atomic staged-bundle install

**type:** feat | **risk:** P0 | **bcps:** 8 | **delta:** MODIFIED

## Context
The launcher script runs at app start while no instance is running — the safe point to install an update. e05s04 extends the trampoline from binary-only `.new` to full bundle install: extract staged bundle to `.staging`, verify MANIFEST, atomically mv the binary, copy assets, self-update the script, and clean up. Any failure leaves the old install untouched.

## Requirements

#### MODIFIED: trampoline handles bundle.new
**Before:** if `$BINARY.new` exists, mv it over `$BINARY`.
**After:** if `$BINARY.bundle.new` exists, extract to `$APP_DIR/.staging`, verify `sha256sum -c MANIFEST.sha256`, install binary (mv), copy assets, self-update script (chess_app.sh.new → mv → exec self), cleanup. Legacy `$BINARY.new` path kept for 0.4.0 clients.

#### ADDED: atomic failure semantics
On any failure (extract, verify, mv), remove `.staging` and `bundle.new`, continue with the old install. Never leaves a half-installed state.

#### ADDED: host-harness test
A throwaway shell harness builds a fake APP_DIR + tiny bundle, runs the trampoline block, asserts install success and failure-path safety.

## Steps
1. Trampoline v2 block in chess_app.sh → verify: `sh -n kindlichess/chess_app.sh && grep -q 'bundle.new' kindlichess/chess_app.sh`
2. Host-harness test → verify: `sh specs/epics/e05-safe-inapp-update/trampoline-harness.sh`
3. Legacy .new path preserved → verify: `grep -q 'BINARY.new' kindlichess/chess_app.sh`
4. Flash scripts sh -n clean → verify: `sh -n flash.sh && sh -n flash_usb.sh`

## Verification Script
1. `sh -n kindlichess/chess_app.sh` — parses clean
2. Run trampoline-harness.sh — install success + failure paths both pass
3. `grep -c 'bundle.new\|BINARY.new' kindlichess/chess_app.sh` — both paths present

## Out of scope
- On-device UAT of a real release (manual, needs k2 on LAN)
- Updating scriptlet/kindlichess.sh in-app (stays the stable entry point)

## Risks
- mv over a running script on fuse.fsp — spike proved binary mv works; script self-update via .new + exec is the same pattern
- Partial extraction on low space — preflight in e05s03 checks space before download; trampoline re-verifies MANIFEST before installing
