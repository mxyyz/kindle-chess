---
story_id: e11s01
title: "Adapter: event stream + held-open board seek (POST /api/board/seek)"
type: feature
risk: P1
context: domain
bcps: 8
delta: ADDED
---

# Story e11s01 — seek + event-stream adapter

## Context

The Play form's "Random" opponent must use the official board seek
`POST /api/board/seek` (https://lichess.org/api#tag/board/POST/api/board/seek),
not `POST /api/challenge/open`. Two API contracts matter (verified against the
vendored litchee 0.2.2 source and the official docs):

1. **The seek connection must stay established** — dropping it cancels the
   seek. litchee models this: `SeekRequest::send()` returns
   `BoxStream<'static, Result<Value>>` (NDJSON); the doc comment says "Hold
   the returned stream open to keep the seek active; dropping it cancels the
   seek."
2. **Open the event stream first** — `GET /api/stream/event`
   (https://lichess.org/api#tag/board/GET/api/stream/event) should be open
   BEFORE the seek starts so the `gameStart` event is not missed.
   litchee: `BoardApi::stream_events()` → stream of `LichessIncomingEvent`
   (`gameStart` shape: `{"type":"gameStart","game":{"gameId":"g",...}}`).

Seek form fields (litchee `SeekForm`): `rated`, `time` (**minutes**, f32),
`increment` (secs), `days`, `variant`, `color`, `ratingRange` — note
`time`/`increment`, NOT `clock.limit`/`clock.increment` (those belong to the
challenge endpoints). Scope: `board:play` (already requested by the app).

## Requirements

#### ADDED: `LitcheeAdapter::stream_events`
Wraps `self.client.board().stream_events()`, maps each `LichessIncomingEvent`
to an app-level event type (`EventStreamEvent` in models/board_api.rs:
`GameStart { game_id }`, `GameFinish { game_id }`, `Challenge`,
`ChallengeCanceled { id }`, `ChallengeDeclined { id }`, `Other`). Requires
auth (token), like the other challenge methods.

#### ADDED: `LitcheeAdapter::seek(mode, rated) -> SeekHandle`
Builds the litchee seek request with form mapping:
- `ChallengeMode::RealTime` → `time=10.0, increment=5` (10:5 default)
- `ChallengeMode::Correspondence(d)` → `days=d`
- `ChallengeMode::Unlimited` → neither
- always `rated(rated)` + `color("random")`
Calls `.send()` → returns the held-open NDJSON `BoxStream<Result<Value>>`
wrapped in a `SeekHandle` struct (so the screen can hold/drop it and read
lines). No timeout — the stream is intentionally long-lived.

#### ADDED: gameStart extraction
`SeekHandle::next_game_start()` (or a free fn `game_id_from_seek_value(&Value)
-> Option<String>`): parse each NDJSON `Value`; when `type == "gameStart"`
extract `game.gameId` (litchee's own test shape) — also accept a bare game
object with `id`/`gameId` for robustness. Pure function → unit-testable
without a network.

#### ADDED: wiremock coverage (new file `crate/tests/board_seek.rs`)
- seek POSTs `/api/board/seek` with `rated=true`, `color=random`,
  `time=10`, `increment=5` for RealTime (body_string_contains assertions);
  Correspondence → `days=3`; Unlimited → no time/increment fields.
- seek stream delivers `{"type":"gameStart","game":{"gameId":"seekGame1",...}}`
  → `game_id_from_seek_value` returns `Some("seekGame1")`; a non-gameStart
  line → None.
- event stream: GET `/api/stream/event` NDJSON lines (gameStart, challenge,
  challengeCanceled) → mapped to `EventStreamEvent` variants.
- anonymous adapter → both methods error with "authentication".

## Implementation Steps

1. `EventStreamEvent` model + `game_id_from_seek_value` pure fn + unit tests → verify: `cd crate && cargo test --lib seek 2>&1 | grep -q 'test result: ok'`
2. `stream_events()` + `seek()`/`SeekHandle` on the adapter + facade methods on `BoardAPI` → verify: `cd crate && cargo build --all-targets 2>&1 | tail -1 | grep -q 'Finished'`
3. wiremock `crate/tests/board_seek.rs` (form fields per mode, gameStart extraction, event stream, auth) → verify: `cd crate && cargo test --test board_seek 2>&1 | grep -q 'test result: ok'`
4. Gates → verify: `cd crate && cargo test 2>&1 | grep -q 'test result: ok' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 | grep -q 'Finished' && cargo fmt --check && echo GATES-OK`

## Out of scope

- Reconnecting a dropped seek automatically (documented risk; the UI cancels
  and the user re-taps Send)
- Forwarding challenge events into InvitesScreen live (Invites keeps polling;
  the event stream is held open during the seek only)
- ratingRange

## Risks

- If the live seek stream shape differs from `gameStart`, `game_id_from_seek_value`
  must tolerate it — the bare-game-object fallback covers it; wiremock pins
  the official shape
- Long-lived HTTP on the Kindle: no read timeout must be set on the seek
  request (litchee's stream request has none by default — keep it that way)

## Acceptance criteria

- [ ] seek POST body carries the right fields per mode + rated + color=random
- [ ] gameStart NDJSON line yields the game id; other lines don't
- [ ] event stream maps gameStart/challenge/canceled/declined
- [ ] anonymous calls error cleanly
- [ ] All gates green
