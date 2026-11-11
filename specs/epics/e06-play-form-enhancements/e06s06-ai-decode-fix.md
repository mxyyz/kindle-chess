---
story_id: e06s06
title: "Bugfix: AI challenge 'failed to decode LichessGame' (raw POST /api/challenge/ai)"
type: fix
risk: P0
context: domain
bcps: 5
delta: MODIFIED
bug: BUG-2026-11-11T122900
---

# Story e06s06 — AI challenge decode fix

## Context

Bug BUG-2026-11-11T122900: Send with Computer selected fails with "API error:
failed to decode LichessGame". Root cause: litchee 0.2.2 parses the
`POST /api/challenge/ai` response as `LichessGame`, whose `variant` is a string
enum, but the live API returns a variant **object**. The wiremock fixture in
`crate/tests/challenge_ai_open.rs` used `"variant":"standard"` and missed it.
Same workaround pattern as `get_ongoing_games`: raw reqwest + parse what we need.

## Requirements

#### MODIFIED: send_challenge_ai uses a raw POST
**Before:** `challenges().challenge_ai(level)...send()` → litchee decode of
`LichessGame` → fails on live API.
**After:** raw `reqwest` POST to `{LICHESS_API_BASE}/challenge/ai` with form
fields `level`, `clock.limit`/`clock.increment` (RealTime), `days`
(Correspondence), `color=random`; parse body as `serde_json::Value`, extract
`id` as String. Return type changes from `LichessGame` to a small local
`AiGame { id: String }` (or just `String`) — update facade + call sites.

#### MODIFIED: wiremock fixture matches the live payload
`challenge_ai_open.rs` AI response uses `"variant":{"key":"standard","name":"Standard","short":"Std"}`
(object, as the real API sends). Assert the id is extracted. Add a negative
test: response without `id` → error.

## Implementation Steps

1. Fix the fixture to object-shaped variant; watch the current test fail (RED) → verify: `cd crate && cargo test --test challenge_ai_open 2>&1 | grep -q 'FAILED\|panicked'`
2. Rewrite `send_challenge_ai` as raw POST returning `AiGame { id }`; update `board.rs` facade and `play.rs`/`chess_game.rs` call sites (`g.id` still works) → verify: `cd crate && cargo build --all-targets 2>&1 | tail -1 | grep -q 'Finished'`
3. Add negative test (no id → error) → verify: `cd crate && cargo test --test challenge_ai_open 2>&1 | grep -q 'test result: ok'`
4. Full gates → verify: `cd crate && cargo test 2>&1 | grep -q 'test result: ok' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 | grep -q 'Finished' && cargo fmt --check && echo GATES-OK`

## Out of scope

- Upstreaming a litchee fix (out of our control; raw call is the established pattern)
- AI game fields beyond id (the game stream provides everything else)

## Risks

- Form field names must match the API exactly (`clock.limit` etc.) — covered by wiremock body assertions

## Acceptance criteria

- [ ] Fixture with object variant passes against the raw-POST implementation
- [ ] Computer send on device returns a game id and opens ChessGameScreen
- [ ] Negative test: missing id errors cleanly
- [ ] All gates green
