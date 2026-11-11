---
story_id: e06s03
title: "Challenge send flow: route to correct litchee call per OpponentKind + rated"
type: feat
risk: P0
context: domain
bcps: 5
delta: MODIFIED
---

# Story e06s03 — Challenge send flow routing

## Context

`PlayScreen::send_challenge` (play.rs ~line 151) currently requires
`self.opponent`, sends via `api.send_challenge(&opponent, &mode)` (casual), and
emits `AppEvent::ChallengeSent(opponent)` → the screen pushes `InvitesScreen`.
With e06s01's adapter methods in place, the send must route per
`OpponentKind` and honor the rated toggle.

## Requirements

#### MODIFIED: Send routes per opponent kind
**Before:** Send always calls `send_challenge(opponent, mode)` (casual) and
pushes InvitesScreen on success.
**After:** `match opponent_kind` — Friend → `send_challenge(name, mode, rated)`
→ `ChallengeSent` → InvitesScreen (unchanged UX). Computer →
`send_challenge_ai(level, mode)` → the returned game starts immediately →
push `ChessGameScreen` via `attach_game(game_id, my_turn)`. Random →
`create_open_challenge(mode, rated)` → `ChallengeSent` → InvitesScreen
(open challenge listed as outgoing).

#### ADDED: Per-kind status messages (EN/DE)
`play-status-sending-ai`, `play-status-sent-ai`/`game started`,
`play-status-open-sent` in `crate/i18n/en.ftl` + `de.ftl`; failure reuses
`play-status-failed`.

#### ADDED: New AppEvent for AI game start
`AppEvent::AiGameStarted { game_id, my_turn }` (events.rs) so the runner can
push `ChessGameScreen`; keeps the event seam used by all screens.

## Implementation Steps

1. Add `opponent_kind: OpponentKind` (default Friend) + `rated: bool` state to
   `PlayScreen`; extend `send_challenge` with the per-kind match → verify: `cd crate && cargo build 2>&1 | tail -1 | grep -q 'Finished'`
2. Add `AppEvent::AiGameStarted` variant; handle it in `play.rs` by pushing
   `ChessGameScreen::new(app.attach_game(game_id, my_turn), config)` → verify: `cd crate && cargo build 2>&1 | tail -1 | grep -q 'Finished'`
3. Status messages per kind with i18n keys → verify: `cd crate && grep -q 'play-status-sending-ai' i18n/en.ftl && grep -q 'play-status-sending-ai' i18n/de.ftl`
4. Unit tests: routing logic picks the right adapter call per kind (mock seam
   where possible), AI start emits `AiGameStarted`, open challenge emits
   `ChallengeSent` → verify: `cd crate && cargo test --lib play 2>&1 | grep -q 'test result: ok'`

## Verification Script (Step-by-Step)

1. `./run-dev.sh` → Play → Friend + Rated → Send: outgoing rated challenge
   appears in Invites
2. Play → Computer level 5 → Send: game starts, ChessGameScreen opens
3. Play → Random → Send: open challenge appears as outgoing in Invites

## Out of scope

- Accept/decline of open challenges (already in InvitesScreen)
- Variants, bots-as-separate-mode (deferred per scope)

## Risks

- AI game start needs the game id from `LichessGame` returned by
  `challenge_ai().send()` — if the field name differs, compile error surfaces
  immediately in step 2
- Random (open) challenge only starts when someone accepts — UX must not
  imply instant start (status message says "waiting")

## Acceptance criteria

- [ ] Friend/Random send with rated flag; Computer sends via challenge_ai
- [ ] AI game opens ChessGameScreen immediately
- [ ] Open challenge listed outgoing in InvitesScreen
- [ ] EN/DE status strings present; `cargo test --lib play` green
