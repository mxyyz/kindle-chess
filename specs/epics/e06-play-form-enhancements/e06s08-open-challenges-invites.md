---
story_id: e06s08
title: "Open challenges (Random) listed + cancellable as outgoing in InvitesScreen"
type: fix
risk: P1
context: domain
bcps: 5
delta: MODIFIED
bug: BUG-2026-11-11T123000
---

# Story e06s08 — open challenges visible in Invites

## Context

Bug BUG-2026-11-11T123000: open challenges created via Random render as blank
outgoing rows (`destUser: null` → `opponent: ""`). They must be listed under
outgoing with a clear label and be cancellable there.

## Requirements

#### ADDED: ChallengeSummary.open flag
`open: bool` on `ChallengeSummary`. Set `true` in `create_open_challenge` and
in `map_challenge` when an outgoing entry has `dest_user == None`.

#### MODIFIED: InvitesScreen outgoing label
**Before:** `format!("{} ({})", c.opponent, c.status)` → `" (created)"` for open.
**After:** open rows show localized `invites-open-challenge` ("Open — random player") +
status, e.g. `Open — random player (created)`.

#### MODIFIED: cancel works for open challenges
Cancel already calls `cancel_challenge(id)` → `POST /api/challenge/{id}/cancel`;
verify with wiremock that an open challenge id is cancellable (no API
difference expected).

#### ADDED: wiremock coverage
`challenge_list.rs`: list body containing an outgoing entry with
`"destUser": null` → decodes, maps to `open: true`, `opponent: ""`. Cancel
POST for that id returns 200.

## Implementation Steps

1. Add `open: bool` to ChallengeSummary + all constructors (map_challenge, create_open, test helpers) → verify: `cd crate && cargo build --all-targets 2>&1 | tail -1 | grep -q 'Finished'`
2. Set open=true where dest_user is None (outgoing) / in create_open mapping → verify: `cd crate && cargo test --lib litchee_adapter 2>&1 | grep -q 'test result: ok'` (after adding wiremock test)
3. Invites label for open rows + i18n `invites-open-challenge` EN/DE → verify: `cd crate && grep -q 'invites-open-challenge' i18n/en.ftl && grep -q 'invites-open-challenge' i18n/de.ftl && cargo test --lib invites 2>&1 | grep -q 'test result: ok'`
4. Wiremock: destUser:null list entry + cancel → verify: `cd crate && cargo test --test challenge_list 2>&1 | grep -q 'test result: ok'`
5. Gates → verify: `cd crate && cargo test 2>&1 | grep -q 'test result: ok' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 | grep -q 'Finished' && cargo fmt --check && echo GATES-OK`

## Out of scope

- Accepting *incoming* open challenges from others (not possible — open
  challenges are accepted via the seek/lobby flow, not invites)
- A separate "browse open challenges" screen (e07 decision: InvitesScreen covers it)

## Risks

- If the live list payload for open entries breaks litchee decode entirely,
  the whole list fails — the wiremock test with a realistic entry proves it
  doesn't

## Acceptance criteria

- [ ] Open challenges appear under outgoing with a clear label
- [ ] Cancel removes them (wiremock proves the call)
- [ ] `open` flag mapped in both create + list paths
- [ ] Gates green
