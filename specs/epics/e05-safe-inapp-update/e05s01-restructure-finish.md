# e05s01 — Restructure finish: flash scripts + run-dev.sh + docs

**type:** refactor | **risk:** P2 | **bcps:** 2 | **delta:** MODIFIED

## Context
The repo restructure (kindle_KUAL removed, bundle at repo-root kindlichess/, crate root renamed to crate/) is committed but flash.sh, run-dev.sh, CLAUDE.md and docs still reference the removed kindle_KUAL/ tree. This story finishes the restructure so deploys are clean before e05 changes the update path.

## Requirements

#### MODIFIED: flash scripts deploy repo-root kindlichess/
**Before:** flash.sh copies binary into `kindle_KUAL/kindlichess/bin/` and scps `kindle_KUAL/*` to `/mnt/us`.
**After:** flash.sh copies binary into `kindlichess/bin/` (mkdir -p) and scps `kindlichess/*` to `/mnt/us`; flash_usb.sh copies `kindlichess/` to the mounted Kindle; token-aside dance targets `kindlichess/secrets/token.json`.

#### MODIFIED: run-dev.sh resolves app dir from repo-root
**Before:** `HELLOKINDLE_DIR="$REPO_ROOT/kindle_KUAL/kindlichess"`.
**After:** `HELLOKINDLE_DIR="$REPO_ROOT/kindlichess"`.

#### MODIFIED: docs reference repo-root layout
**Before:** CLAUDE.md, ARCHITECTURE.md, ci-cd.md mention `kindle_KUAL/kindlichess/` and `kindle_KUAL/extensions/hellokindle/menu.json`.
**After:** all reference `kindlichess/` at repo root; scriptlet-only launch documented; no kindle_KUAL/hellokindle refs outside historical artifacts.

## Steps
1. Update flash.sh deploy paths → verify: `sh -n flash.sh && ! grep -q kindle_KUAL flash.sh`
2. Update flash_usb.sh deploy paths → verify: `sh -n flash_usb.sh && ! grep -q kindle_KUAL flash_usb.sh`
3. Update run-dev.sh HELLOKINDLE_DIR → verify: `grep -q 'kindlichess' run-dev.sh && ! grep -q kindle_KUAL run-dev.sh && bash -n run-dev.sh`
4. Docs sweep (CLAUDE.md, ARCHITECTURE.md, ci-cd.md, tech-stack.md, api/README.md) → verify: `! grep -rn 'kindle_KUAL\|hellokindle' CLAUDE.md docs/ specs/tech-architecture/ | grep -v 'specs/epics\|CHANGELOG'`
5. Crate still green → verify: `cd crate && cargo test 2>&1 | grep -q 'test result: ok'`

## Verification Script
1. `sh -n flash.sh && sh -n flash_usb.sh` — both parse clean
2. `grep -c kindle_KUAL flash.sh flash_usb.sh run-dev.sh` — 0 matches
3. `cd crate && cargo test` — all tests pass

## Out of scope
- Changing the on-device layout beyond the rename (already done in the restructure commit)
- Rewriting historical artifacts (specs/epics/, CHANGELOG.md)

## Risks
- Token-aside dance broken → flash scripts must still mv token.json aside and restore it; verify by inspection
