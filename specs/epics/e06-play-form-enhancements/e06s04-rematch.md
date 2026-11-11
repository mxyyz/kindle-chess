---
story_id: e06s04
title: "Rematch button on ChessGameScreen game-over: re-challenge same opponent with same settings"
type: feat
risk: P1
context: domain
bcps: 3
delta: ADDED
---

# Story e06s04 — Rematch on game over

## Context

`ChessGameScreen` (`crate/src/ui/screens/chess_game.rs`) receives
`GameOverEvent` from the game stream (models/board_api.rs:131; the stream
closes with "game finished" at chess_game.rs:283). The screen currently shows
only the final position. Rematch = send a new challenge to the same opponent
with the same mode + rated (Lichess rematch is informational; a new challenge
is the real mechanism — verified in planning-context prior art).

**Zoom-out:** `chess_game.rs` owns the in-game UI; caller is the app runner
via the screen stack. It holds `app: ChessApp` (`models/chess.rs`) which has
`game_id()` and `turn()`. The last challenge context (opponent kind, name,
mode, rated) must be carried into the screen when it is pushed (e06s03) —
add an optional `rematch: Option<RematchContext>` field.

## Requirements

#### ADDED: RematchContext carries the original challenge
`RematchContext { kind: OpponentKind, mode: ChallengeMode, rated: bool }`
(plain data, models/board_api.rs). `ChessGameScreen::new` gains a param (or
builder setter) so PlayScreen/InvitesScreen can attach it.

#### ADDED: Rematch button on game over
When a `GameOverEvent` arrives (or the stream reports finished), show a
"Rematch" button in the sidebar. On tap: re-issue the challenge per kind —
Friend → `send_challenge(name, mode, rated)`; Computer → `send_challenge_ai`
(same level); Random → `create_open_challenge(mode, rated)` — and navigate
like e06s03 (AI → new ChessGameScreen; others → InvitesScreen).

#### ADDED: EN/DE strings
`game-rematch`, `game-rematch-sending` in `crate/i18n/en.ftl` + `de.ftl`.

## Implementation Steps

1. Add `RematchContext` to models/board_api.rs; add `rematch` field + setter to
   `ChessGameScreen`; thread it from the push sites → verify: `cd crate && cargo build --all-targets 2>&1 | tail -1 | grep -q 'Finished'`
2. On `GameOverEvent`/stream-finished, set `game_over = true` and render the
   Rematch button in the sidebar; on tap, spawn the re-challenge per kind → verify: `cd crate && cargo build 2>&1 | tail -1 | grep -q 'Finished'`
3. i18n keys in en.ftl + de.ftl → verify: `cd crate && grep -q 'game-rematch' i18n/en.ftl && grep -q 'game-rematch' i18n/de.ftl`
4. Unit test: game-over detection sets the flag; rematch params equal the
   original `RematchContext` → verify: `cd crate && cargo test --lib chess_game 2>&1 | grep -q 'test result: ok'`

## Verification Script (Step-by-Step)

1. `./run-dev.sh` → finish a game (or force game-over in dev harness) →
   Rematch button appears in the sidebar
2. Tap Rematch → for a friend game, a new outgoing challenge to the same
   opponent with the same mode/rated appears in Invites

## Out of scope

- Rematch against anonymous open-challenge opponents (no stable username)
- Auto-accept of the opponent's rematch

## Risks

- Games opened from OngoingGames (not from Play) have no RematchContext —
  button hidden in that case (documented)

## Acceptance criteria

- [ ] Rematch button shown only on game over and only when context exists
- [ ] Re-challenge uses same opponent/mode/rated (or same AI level)
- [ ] EN/DE strings present; `cargo test --lib chess_game` green
