---
story_id: e06s09
title: "Final e06 verification: all gates + cross build note + docs review"
type: chore
risk: P2
context: infra
bcps: 2
delta: ADDED
---

# Story e06s09 — final verification

## Context

Closing verification after e06s06–e06s08: full gates, docs consistency, and the
manual verification script for the user.

## Requirements

#### ADDED: full gate run + docs check
build/test/clippy/fmt on host **plus** the armv7 musl cross build (verified
locally: `env -u LD_LIBRARY_PATH RUSTFLAGS="-C target-feature=+crt-static" cross build --target armv7-unknown-linux-musleabi --release` — agent sandboxes must unset LD_LIBRARY_PATH or crun fails on the bundled libstdc++). Docs
(`docs/ARCHITECTURE.md`, `crate/src/api/README.md`) reflect the final Play
layout and open-challenge behavior.

## Implementation Steps

1. Gates → verify: `cd crate && cargo build 2>&1 | tail -1 | grep -q 'Finished' && cargo test 2>&1 | grep -q 'test result: ok' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 | grep -q 'Finished' && cargo fmt --check && env -u LD_LIBRARY_PATH RUSTFLAGS="-C target-feature=+crt-static" cross build --target armv7-unknown-linux-musleabi --release 2>&1 | tail -1 | grep -q 'Finished' && echo GATES-OK`
2. Docs mention final layout + open challenges → verify: `grep -q 'Open — random player\|challenge/open' docs/ARCHITECTURE.md && grep -q 'challenge/open' crate/src/api/README.md`

## Verification Script (Step-by-Step)

1. `./run-dev.sh` → Play: kind row on top (Computer | Friend | Random), selector
   row below (level/days/info), mode row, then centered Send in the lower half
2. Computer + level 5 → Send → game opens (bug fixed)
3. Random → Send → outgoing row in Invites reads "Open — random player (created)";
   Cancel removes it
4. Friend → tap Friend button → friendlist opens → pick → button shows the name

## Out of scope

- On-device UAT on the k2 (manual, needs device on LAN)

## Risks

- In agent sandboxes the cross build needs `env -u LD_LIBRARY_PATH` (sandbox
  LD_LIBRARY_PATH breaks crun); in a normal shell it works as-is

## Acceptance criteria

- [ ] All host gates green
- [ ] Docs match final behavior
- [ ] Manual script above passes
