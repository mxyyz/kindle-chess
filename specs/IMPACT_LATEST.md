## Target

Random-opponent flow in the Play form (`OpponentKind::Random`): replace
`create_open_challenge` (`POST /api/challenge/open`, litchee `create_open()`)
with the official board seek (`POST /api/board/seek`, litchee `BoardApi::seek()`
— held-open NDJSON stream, dropping it cancels the seek) plus the event stream
(`GET /api/stream/event`, litchee `stream_events()`) opened before the seek.

## Dependents (5)

- `crate/src/api/board.rs:66` — facade `create_open_challenge` (calls adapter)
- `crate/src/ui/screens/play.rs:382` — Random arm of `kick_send`
- `crate/src/ui/screens/play.rs:331` — `send_event` maps Random → `ChallengeSent("")`
- `crate/src/ui/screens/chess_game.rs:121-122` — Random arm of `kick_rematch`
- `crate/tests/challenge_ai_open.rs` — wiremock tests for create_open

## Affected Stories

- e06s03 (send flow routing) — MODIFIED: Random arm becomes seek, not create_open
- e06s04 (rematch) — MODIFIED: Random rematch becomes seek
- e06s08 (open challenges in Invites) — retained: `open` flag/label still correct
  for genuine `/api/challenge/open` entries (e.g. created on web); Play no
  longer creates them

## Test Coverage

- `crate/tests/challenge_ai_open.rs` — create_open POST body + auth
- `crate/tests/challenge_list.rs` — list decode incl. `destUser: null` open entry + cancel
- `crate/src/ui/screens/play.rs` tests — send_event per kind, send routing seams
- `crate/src/ui/screens/chess_game.rs` rematch_tests — context/visibility
- Gap: no coverage for held-open seek stream, event-stream parsing, seek cancel
  (new in e11)

## Risk: Medium

Shared send-flow seams (play.rs + chess_game.rs both route per OpponentKind) are
touched, but all have unit/wiremock coverage and the change is additive at the
adapter layer (new `seek`/`stream_events` methods) with the two call sites
switched in one place each.

## Recommended action

Proceed with e11 plan; add wiremock tests for seek form fields + gameStart
extraction + event-stream parsing before touching the screens (RED first).
