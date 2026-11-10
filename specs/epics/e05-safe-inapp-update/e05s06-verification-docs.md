# e05s06 — Verification + docs (END of epic)

**type:** chore | **risk:** P1 | **bcps:** 3 | **delta:** ADDED

## Context
Final story of e05: proves all local gates green, adds a CI bundle dry-run, updates docs to describe the bundle flow, and smoke-tests run-dev.sh path resolution.

## Requirements

#### ADDED: full local gates green
cargo build (dev + armv7 cross) + test + clippy -D warnings + fmt --check.

#### ADDED: CI bundle dry-run in rust.yml
Every PR proves the bundle contract: build MANIFEST + tar + extract + sha256sum -c.

#### ADDED: docs describe the bundle flow
ARCHITECTURE.md, ci-cd.md, tech-stack.md, CLAUDE.md document update.tar.gz, preflight, trampoline install, asset contract.

## Steps
1. Full local gates → verify: `cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check && cross build --target armv7-...`
2. rust.yml bundle dry-run step → verify: `grep -q MANIFEST.sha256 .github/workflows/rust.yml`
3. Docs sweep → verify: `grep -q update.tar.gz docs/ARCHITECTURE.md docs/ci-cd.md specs/tech-architecture/tech-stack.md CLAUDE.md`
4. run-dev.sh smoke → verify: `bash -n run-dev.sh && grep -q kindlichess run-dev.sh`

## Verification Script
1. Run all gates locally — all green
2. `grep -q update.tar.gz` in all 4 doc files
3. CI pipeline on the e05 branch — bundle dry-run job passes

## Out of scope
- On-device UAT (manual)
- Removing legacy assets

## Risks
- Cross build requires Docker/podman in the dev environment — use `env -u LD_LIBRARY_PATH CROSS_CONTAINER_ENGINE=docker` per spike
