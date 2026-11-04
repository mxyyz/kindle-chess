# BUG-2026-11-04T184513: Puzzle solutions are never posted to Lichess

## Problem

- **Actual:** Puzzles are only fetched and played locally. When a puzzle is
  finished (`PuzzleStatus::Solved`), nothing is sent to Lichess, so the
  account's server-side solved-set and puzzle activity never advance. Two
  user-visible symptoms follow:
  1. A finished daily puzzle is never recorded, so the home-screen puzzle
     picker (which checks the last 24 h of `GET /api/puzzle/activity` for the
     daily's id) keeps serving the same daily puzzle forever.
  2. The sidebar's "Next puzzle" button calls `GET /api/puzzle/next` with the
     current filters; Lichess keeps returning the same puzzle until the current
     one is marked solved server-side, so with unchanged filters the button
     appears broken and refetches the same puzzle.
- **Expected:** When a puzzle is solved locally and the user is signed in, the
  result is posted via `POST /api/puzzle/batch/{angle}`
  (`{"solutions": [{"id": ..., "win": true, "rated": true}]}`). After that,
  `/puzzle/next` advances to a fresh puzzle and the daily shows up in puzzle
  activity, so the home screen moves on.
- **Reproduce:** Sign in, open the daily puzzle, play the full solution, then
  press "Next puzzle" — the same puzzle is refetched. Check
  `GET /api/puzzle/activity` on the web — the solved puzzle is absent.

Security impact: NONE — no exploit path; this is a missing outbound API call
with the user's own bearer token, same as the existing puzzle reads.

## Root Cause Analysis

Verified by direct code inspection (no code path exists):

- A repo-wide search for any solve/batch submission finds no `POST` to
  `/api/puzzle/batch/{angle}` (or any solve endpoint) anywhere in `src/`.
  litchee 0.2.2 *does* expose it (`PuzzlesApiAccessor::solve_batch(angle,
  &[LichessPuzzleSolution], nb)`), but the adapter never calls it.
- `PuzzleSession::new` drops the puzzle `id` — it keeps rating/themes/FEN/
  solution but not the id, so the session cannot even identify what it solved.
- `PuzzleScreen` handles the transition to `Solved` by only updating the
  sidebar status text; no network task is spawned.
- Contributing factor: `GET /api/puzzle/next` is stable per (account, filters)
  until the current puzzle is solved server-side, and the home-puzzle picker
  relies on `GET /api/puzzle/activity`, which only records *posted* solves.
  Both were designed assuming solutions get posted — that piece was never
  implemented.
- Enablers already in place: `puzzle:write` is a default OAuth scope
  (`AuthConfig::default`), and the `PuzzleFetcher` mock + wiremock integration
  seams exist for testing.

Risk level: Low — additive change; anonymous users are unaffected (post is
skipped without a token), and a 403 from tokens issued before `puzzle:write`
must degrade quietly (log + continue), matching the existing `/puzzle/next`
fallback pattern.

## TDD Fix Plan

1. **RED**: `PuzzleSession` retains the puzzle id — test that
   `PuzzleSession::new(puzzle).id == puzzle.id` for every existing test fixture.
   **GREEN**: add a `pub id: String` field to `PuzzleSession`, populated in
   `new()`.
   **verify**: `cargo test --lib models::puzzle`

2. **RED**: adapter maps a solved puzzle to a batch-solve POST — wiremock
   integration test: stand up `MockServer`, `LitcheeAdapter::with_base_url`,
   call the new `post_puzzle_solution("mix", "test123", true, 0)`, assert
   `method("POST")`, `path("/api/puzzle/batch/mix")`, `query_param("nb", "0")`,
   and body JSON `{"solutions":[{"id":"test123","win":true,"rated":true}]}`;
   respond with a `LichessPuzzleSolveResponse` body and assert the decoded
   `rounds[0].win == Some(true)`.
   **GREEN**: `LitcheeAdapter::post_puzzle_solution(angle, id, win, nb)`
   calling `client.puzzles().solve_batch(angle, &[LichessPuzzleSolution], nb)`
   and mapping the response to app types.
   **verify**: `cargo test --test lib api_puzzle`

3. **RED**: api-layer wrapper resolves the angle from `PuzzleParams`
   (`PuzzlePhase::Any` → `"mix"`, otherwise the phase's theme key) and returns
   `Ok(None)` without a token instead of erroring.
   **GREEN**: `api::post_puzzle_solution(token: Option<TokenInfo>, params,
   id, win)` in `api/api.rs`, mirroring `get_next_puzzle`'s
   authenticated-with-anonymous-fallback shape (here: no token → `Ok(None)`;
   403 → warn + `Ok(None)` so old tokens don't break local play).
   **verify**: `cargo test --test lib api_puzzle` + `cargo test --lib`

4. **RED**: solving a puzzle triggers exactly one post when signed in —
   session-level test: drive `try_move`/`apply_opponent_reply` to `Solved` and
   assert the session exposes `id` + `status == Solved` (the screen-level trigger
   is a thin wiring step: on transition to `Solved` with `token.is_some()`,
   spawn the post; covered by the integration tests above plus a unit test on
   a small `should_post(token, status)` helper if extracted).
   **GREEN**: in `PuzzleScreen`'s `MoveMade`/`PuzzleOpponentMove` handlers,
   after a transition to `Solved`, spawn a tokio task calling
   `post_puzzle_solution(self.token.clone(), self.params, session.id.clone(),
   true)`; log success/failure, never block the UI.
   **verify**: `cargo test` (full suite) + `cargo build`

**REFACTOR**: none anticipated beyond keeping the mock/wiremock helpers
shared; do not remove the `PuzzleFetcher` GET seam (still used by fetch tests).

## Acceptance Criteria

- [ ] Signing in and solving a puzzle results in one
      `POST /api/puzzle/batch/{angle}` with that puzzle's id and `win: true`.
- [ ] After solving, `GET /api/puzzle/next` (unchanged filters) returns a
      *different* puzzle, and the daily appears in `/api/puzzle/activity`.
- [ ] Anonymous use (no token) still works; no post attempted, no error shown.
- [ ] Tokens without `puzzle:write` (403) degrade quietly to local-only play.
- [ ] All new tests pass; existing 76 lib + integration tests still pass.

## Resolution

<!-- filled in by validate-fix -->
