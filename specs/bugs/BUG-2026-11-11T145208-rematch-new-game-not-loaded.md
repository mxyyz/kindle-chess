---
bug_id: BUG-2026-11-11T145208
date: 2026-11-11
severity: high
priority: high
scope: e06s04-rematch
status: fixed  # attach_game re-scopes in-game backends via detach(); RED test confirmed oldGame kept, GREEN after fix
summary: >-
  Rematch after a game starts the new game on Lichess (POST succeeds, new game
  id visible in logs and on lichess.org) but the board never loads the new
  game — the pushed game screen stays scoped to the old, finished game.
files:
  - crate/src/app/chess.rs
  - crate/src/api/board.rs
  - crate/src/ui/screens/chess_game.rs
  - crate/src/ui/screens/invites.rs
---

# BUG-2026-11-11T145208 — rematch starts a new game but the board doesn't load it

## Problem

**Actual:** Play → Computer → Send → game over → tap Rematch. Logs show the
`POST /api/challenge/ai` succeeded and `Rematch AI game started: <newId>` was
emitted, and lichess.org shows the new game — but the board keeps showing the
old finished game (or never updates to the new one).
**Expected:** the rematch pushes a game screen scoped to the NEW game id, so
the board stream loads the new game's position.

**AI-only?** No. The same defect hits player rematches: Friend/Random rematch
pushes InvitesScreen while the app is still in-game; when the opponent accepts,
the accepted game is attached with the same no-op path, so the new player game
also fails to load. Only flows where the app is Idle when attaching (Play send,
Invites from Home/Play, Ongoing-games pick) work.

## Root Cause Analysis (4-phase)

1. **Reproduce:** device, AI game to completion, tap Rematch. Log evidence:
   `Rematch AI game started: <newId>` followed by
   `attach_game called on non-idle backend; ignored` — the new game id is
   dropped on the floor.
2. **Isolate:** the transition `ChessApp::attach_game` only maps
   `OnlineIdle → OnlineInGame`. An `OnlineInGame` backend (any rematch,
   since the rematch button only exists on a game screen) hits the catch-all
   arm, which warns and returns the backend unchanged — still scoped to the
   OLD game id. The rematch handler in the game screen then pushes a new game
   screen whose stream task subscribes to the OLD game.
3. **Hypotheses:**
   - H1 (root cause): `attach_game` is a no-op on in-game backends, so the
     pushed screen streams the old game id. Falsification test: attach game B
     after game A on the same app, assert the in-game api reports game B.
   - H2: the pushed screen never starts its stream. Falsified: render kicks
     the stream on first paint (`stream_started` guard), and the old game's
     position still arrives — only the game id is wrong.
   - H3: wire/decode failure on the new game's stream. Falsified: the AI
     decode path was fixed in e06s06 and the new game id never even reaches
     the stream (see H1).
4. **Verify:** RED unit test below — `attach_game` from an in-game app keeps
   the old game id (fails before the fix, passes after). Single root cause:
   the Idle-only transition in `attach_game`.

Security impact: NONE — local state-scoping bug, no data exposure or injection
path.

## TDD Fix Plan

1. **RED**: `app/chess.rs` test — build online app, attach game A, attach game B,
   assert in-game api `game_id() == "B"` and turn snapshot from B.
   verify: `cd crate && cargo test --lib attach_game 2>&1 | grep -q FAILED`
2. **GREEN**: add `BoardAPI<InGame>::detach(self) -> BoardAPI<Idle>` (keeps
   token/user/adapter, drops game scope) and make `ChessApp::attach_game`
   handle `OnlineInGame` via `api.detach().attach_game(...)`. Offline still
   warns + no-ops.
   verify: `cd crate && cargo test --lib attach_game 2>&1 | grep -q 'test result: ok'`
3. **RED→GREEN (screen flow)**: rematch handler seam — extract the
   "re-scope app to new game" step so a test proves a rematch from an in-game
   app yields the new game id on the pushed screen's app (AI path), and the
   InvitesScreen accept path benefits from the same `attach_game` fix
   (player path).
   verify: `cd crate && cargo test --lib rematch 2>&1 | grep -q 'test result: ok'`
4. **GREEN (hygiene)**: in the game screen's rematch-accept handler, cancel the
   old stream task before pushing the new screen (Push does not stop the old
   task; the old game is terminal so its stream is usually dead, but a
   mid-backoff task must not keep emitting old-game events).
5. **Gates**: `cd crate && cargo test 2>&1 | grep -q 'test result: ok' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 | grep -q 'Finished' && cargo fmt --check && echo GATES-OK`

## Acceptance Criteria

- [ ] Rematch vs AI loads the new game id on the board (unit: attach from in-game re-scopes)
- [ ] Rematch vs player loads the new game after accept (same attach_game path)
- [ ] Old Play/Invites/Ongoing attach flows unchanged (idle path untouched)
- [ ] All gates green

## Resolution

Fixed 2026-11-11 on branch e06-play-form-enhancements:
- `BoardAPI<InGame>::detach()` returns to the Idle scope keeping token/user/adapter.
- `ChessApp::attach_game` now handles `OnlineInGame` via `detach().attach_game(..)`,
  so rematch (AI immediately; player after accept in InvitesScreen) re-scopes to the
  NEW game id. Idle and Offline paths unchanged.
- Rematch handlers in the game screen also cancel the old stream task before Push.
- Tests: `attach_game_from_in_game_rescopes_to_new_game` (RED before: game_id stayed
  "oldGame"), `in_game_detach_returns_to_idle_and_reattaches_to_new_game`,
  `rematch_app_rescopes_in_game_backend_to_new_game`. Full gates green.
