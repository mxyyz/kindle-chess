---
story_id: e11s03
title: "Rematch with kind=Random re-issues a seek instead of create_open_challenge"
type: feature
risk: P2
context: domain
bcps: 3
delta: MODIFIED
---

# Story e11s03 — Random rematch via seek

## Context

**Before (e06s04):** `kick_rematch` with `OpponentKind::Random` calls
`api.create_open_challenge(&ctx.mode, ctx.rated)` and the result arrives as
`ChallengeSent("")` → InvitesScreen.

**After (e11):** Random rematch uses the same seek helper as the Play flow
(shared `pub(crate) fn kick_seek(api, mode, rated, tx)` in play.rs, reused by
chess_game.rs), emitting `AiGameStarted` on match — the existing
`AiGameStarted` handler in chess_game.rs already attaches the new game and
pushes a fresh ChessGameScreen (and e06s04/BUG-2026-11-11T145208 made that
re-scope correctly).

## Requirements

#### MODIFIED: chess_game.rs `kick_rematch` Random arm
**Before:** `OpponentKind::Random => api.create_open_challenge(...)` →
`ChallengeSent("")`.
**After:** `OpponentKind::Random => PlayScreen::kick_seek(api, &ctx.mode,
ctx.rated, tx)` → `AiGameStarted` on match (handled 15 lines below).
`create_open_challenge` stays on the adapter (still used by wiremock tests and
valid for legacy/web-created open challenges) but has no UI caller anymore.

## Implementation Steps

1. Extract `kick_seek` as a shared `pub(crate)` helper in play.rs; chess_game
   Random arm calls it → verify: `cd crate && cargo build --all-targets 2>&1 | tail -1 | grep -q 'Finished'`
2. Unit test: rematch with Random context routes through the seek helper
   (assert the emitted event on match is AiGameStarted, not ChallengeSent) → verify: `cd crate && cargo test --lib 2>&1 | grep -q 'test result: ok'`
3. Gates → verify: `cd crate && cargo test 2>&1 | grep -q 'test result: ok' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 | grep -q 'Finished' && cargo fmt --check && echo GATES-OK`

## Acceptance criteria

- [ ] Random rematch starts a seek; match opens the new game directly
- [ ] No `create_open_challenge` call from any UI path
- [ ] Gates green
