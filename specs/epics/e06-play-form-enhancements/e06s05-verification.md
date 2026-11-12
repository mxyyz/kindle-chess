---
story_id: e06s05
title: "Verification: local gates green, docs updated"
type: chore
risk: P2
context: infra
bcps: 2
delta: ADDED
---

# Story e06s05 — Verification + docs

## Context

Closing story for e06: prove all local gates are green and the new challenge
flow is documented where maintainers look.

## Requirements

#### ADDED: Full local gate run
`cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check`, and the armv7 musl cross build (as in CI
`.github/workflows/rust.yml`).

#### ADDED: Docs
`docs/ARCHITECTURE.md` and `crate/src/api/README.md` describe the
OpponentKind routing (friend / challenge_ai / create_open), the rated flag,
and that AI games are always casual.

## Implementation Steps

1. Run full gates → verify: `cd crate && cargo build 2>&1 | tail -1 | grep -q 'Finished' && cargo test 2>&1 | grep -q 'test result: ok' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 | grep -q 'Finished' && cargo fmt --check && echo GATES-OK`
2. Cross build → verify: `cd crate && RUSTFLAGS="-C target-feature=+crt-static" cross build --target armv7-unknown-linux-musleabi --release 2>&1 | tail -1 | grep -q 'Finished'`
3. Update docs → verify: `grep -q 'challenge_ai' docs/ARCHITECTURE.md && grep -q 'OpponentKind' crate/src/api/README.md`

## Verification Script (Step-by-Step)

1. Run the verify command from step 1 — prints GATES-OK
2. Cross build finishes for armv7
3. `docs/ARCHITECTURE.md` mentions the three challenge paths

## Out of scope

- On-device UAT on the k2 (manual, needs device on LAN)
- CI changes (workflow already runs the same gates)

## Risks

- Cross build needs Docker/Podman with `cross` — if unavailable locally, CI
  is the fallback gate; note it in the PR

## Acceptance criteria

- [ ] All local gates green (build/test/clippy/fmt + cross)
- [ ] Docs mention OpponentKind + challenge_ai routing
