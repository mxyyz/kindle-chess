---
story_id: e06s01
title: "Challenge model + litchee adapter: OpponentKind enum, rated flag, challenge_ai, create_open"
type: feat
risk: P0
context: domain
bcps: 8
delta: MODIFIED
---

# Story e06s01 — Challenge model + litchee adapter extensions

## Context

The Play form currently supports only casual standard challenges against friends
(`send_challenge` hardcodes `.rated(false)`, random color). e06 needs three
opponent kinds and a rated flag. litchee 0.2.2 signatures verified against the
vendored source (`~/.cargo/registry/src/*/litchee-0.2.2/src/api/gameplay/`):

- `challenges().challenge(username)` → `ChallengeRequest` with `.rated(bool)`
  (challenges.rs:200), `.clock(limit,inc)`, `.days(u32)`, `.color(...)`,
  `.send() -> LichessChallenge` — `POST /api/challenge/{username}`
- `challenges().challenge_ai(level: u8)` → `AiChallengeRequest` with `.clock`,
  `.days`, `.color`, `.variant`, `.fen`, `.send() -> LichessGame` —
  `POST /api/challenge/ai`. **No `.rated()` exists on AiChallengeRequest:
  Lichess AI games are always casual.** The rated flag therefore applies to
  Friend and Random (open) challenges only.
- `challenges().create_open()` → `OpenChallengeRequest` with `.rated(bool)`
  (challenges.rs:403), `.clock`, `.days`, `.variant`, `.name` —
  `POST /api/challenge/open`.
- `board().seek()` → `SeekRequest` with `.rated(bool)` (board.rs:207) —
  kept as prior art; e06 uses `create_open` per the sliced story
  (open challenge appears in InvitesScreen), not seek.

**Zoom-out:** `models/board_api.rs` owns app-side domain types
(`ChallengeMode`, `ChallengeSummary`, `ALLOWED_CORRESPONDENCE_DAYS`);
callers are `api/litchee_adapter.rs`, `api/board.rs`, and the screens
(`play.rs`, `invites.rs`, `ongoing_games.rs`). `api/litchee_adapter.rs` wraps
the litchee client and maps `LichessChallenge` → `ChallengeSummary`; caller is
the `BoardAPI` facade in `api/board.rs` with `ChessError` mapping.

## Requirements

#### ADDED: OpponentKind enum
`OpponentKind { Friend(String), Computer(u8), Random }` in
`models/board_api.rs`. `Computer` carries the AI level 1–8.

#### MODIFIED: Challenge parameters carry a rated flag
**Before:** `send_challenge(username, &ChallengeMode)` hardcodes
`rated=false`, random color; only the time control varies.
**After:** `send_challenge(username, mode, rated: bool)` passes
`.rated(rated)` to `ChallengeRequest`; `ChallengeSummary` gains a `rated: bool`
field mapped from `LichessChallenge.rated` (defaults false).

#### ADDED: AI challenge adapter method
`send_challenge_ai(level: u8, mode: &ChallengeMode) -> Result<LichessGame-ish,
LichessError>` via `challenges().challenge_ai(level)` with clock/days applied
like friend challenges and random color. Always casual (no rated param —
the litchee type has none). Returns the created game id so the UI can open
`ChessGameScreen` immediately.

#### ADDED: Open challenge adapter method
`create_open_challenge(mode: &ChallengeMode, rated: bool) -> Result<ChallengeSummary, LichessError>`
via `challenges().create_open().rated(rated)` + clock/days. Maps to
`ChallengeSummary` with `incoming=false` so it shows as outgoing in
InvitesScreen.

#### ADDED: BoardAPI facade methods
`api/board.rs` exposes `send_challenge_ai` / `create_open_challenge` with
`ChessError` mapping, mirroring `send_challenge`.

## Implementation Steps

1. Add `OpponentKind` enum + `rated` field on `ChallengeSummary` in
   `models/board_api.rs`; unit tests for enum/serde round-trip → verify: `cd crate && cargo test --lib board_api 2>&1 | grep -q 'test result: ok'`
2. Extend `litchee_adapter.rs::send_challenge` with `rated: bool` param
   (`.rated(rated)` instead of hardcoded `false`); update `map_challenge` to
   carry `rated` → verify: `cd crate && cargo build 2>&1 | tail -1 | grep -q 'Finished'`
3. Add `send_challenge_ai(level, mode)` and `create_open_challenge(mode, rated)`
   to `litchee_adapter.rs` per the verified litchee signatures above → verify: `cd crate && cargo build 2>&1 | tail -1 | grep -q 'Finished'`
4. Expose both on the `BoardAPI` facade in `api/board.rs` with ChessError mapping;
   fix existing `send_challenge` call sites for the new signature → verify: `cd crate && cargo build --all-targets 2>&1 | tail -1 | grep -q 'Finished'`
5. wiremock tests (dev-dep wiremock 0.6 already present): rated=true sends
   `rated=true` in the form body to `/api/challenge/{user}`; `challenge_ai`
   POSTs to `/api/challenge/ai` with `level`; `create_open` POSTs to
   `/api/challenge/open` with rated/clock/days; `ChallengeSummary.rated`
   mapping preserves the flag → verify: `cd crate && cargo test --lib litchee_adapter 2>&1 | grep -q 'test result: ok'`

## Verification Script (Step-by-Step)

1. `cd crate && cargo test --lib board_api litchee_adapter` — all green
2. `cd crate && cargo build --all-targets` — compiles with new signatures
3. `cd crate && cargo test` — full suite green (no regressions in existing
   challenge tests)

## Out of scope

- Game variants (Chess960 etc.) — deferred per scope; `.variant()` not used
- `board().seek()` lobby matchmaking — create_open chosen instead
- UI changes (e06s02/e06s03)

## Risks

- litchee `AiChallengeRequest` has no `rated()` — passing rated for AI would
  be a compile error; AI stays casual (documented in code comment)
- `create_open` response shape differs from `challenge` — covered by wiremock
  test on the mapping

## Acceptance criteria

- [ ] `OpponentKind` exists with Friend/Computer/Random variants
- [ ] `send_challenge` accepts and forwards `rated`
- [ ] `send_challenge_ai` + `create_open_challenge` exist on adapter and BoardAPI
- [ ] wiremock proves correct path + form params for all three calls
- [ ] `cargo test` fully green
