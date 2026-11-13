# BUG-2026-11-13T014911: board switching loop when hopping between ongoing games

## Problem

When the user opens an ongoing game (game A), goes back, then opens another
ongoing game (game B), the board on game B's screen gets stuck in a loop
where it keeps re-rendering game A's position alternating with game B's —
without the user selecting another game. Switching between just 2 games is
enough to reproduce.

Expected: after opening game B, only game B's stream events update the board.

## Security impact

NONE — no security exploit path identified. This is a UI/state bug: stale
background tasks feed events into a shared channel.

## Root Cause Analysis

The `ChessGameScreen` starts a resilient SSE stream task on first render
(`kick_game_stream`). The task has a reconnect loop gated by a
`cancel: Arc<AtomicBool>` flag stored in the screen's `stream_cancel` field.

The cancel flag is set to `true` only in two places:
1. `ChallengeSent` handler (rematch path) — cancels the old stream before the
   rematch pushes a new screen.
2. `DeviceWake` handler — cancels the stalled stream before restarting.

When the user exits the game screen via `ExitToMenu` → `Transition::Pop`,
the app loop pops the screen from the stack. Neither the `ExitToMenu`
handler nor a `Drop` impl sets `stream_cancel` to `true`. The stream task
for the old game keeps running its reconnect loop indefinitely, sending
`GameFullReceived` / `TurnChanged` events into the shared `event_tx` channel.

When a new `ChessGameScreen` is pushed (for game B), it receives events from
BOTH the old game's stream AND the new game's stream. The `GameFullReceived`
and `TurnChanged` handlers don't check which game the event belongs to —
they apply the board position unconditionally. Result: the board alternates
between game A and game B positions → the observed switching loop.

Risk level: Medium (stale background tasks, shared-channel event pollution;
no data corruption but confusing UX and wasted network/CPU).

## TDD Fix Plan

1. **RED**: Write a unit test that constructs a `ChessGameScreen`, simulates
   `stream_started = true` with a cancel flag, then drops the screen and
   asserts the cancel flag is set to `true`.
   **GREEN**: Implement `Drop for ChessGameScreen` that sets
   `stream_cancel` to `true`.
   **verify**: `cd crate && cargo test --lib chess_game`

2. **RED**: Write a test verifying that `ExitToMenu` transition from a
   screen with an active stream cancels it (Drop fires on Pop).
   **GREEN**: Covered by the Drop impl — no additional code needed.
   **verify**: `cd crate && cargo test --lib`

**REFACTOR**: None needed — the Drop impl is the minimal robust fix.

## Acceptance Criteria

- [ ] Dropping a ChessGameScreen cancels its game stream task
- [ ] Switching between 2 ongoing games no longer causes a board switching loop
- [ ] All existing tests still pass
- [ ] clippy + fmt green

## Resolution

Fixed 2026-11-13: added `impl Drop for ChessGameScreen` that sets
`stream_cancel` to true, so the SSE stream task exits its reconnect loop
when the screen is popped. Two regression tests: `drop_cancels_game_stream`
and `drop_without_stream_is_noop`. 258 tests / clippy / fmt green.
