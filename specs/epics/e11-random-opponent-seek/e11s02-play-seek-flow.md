---
story_id: e11s02
title: "Play flow: Random -> event stream first, then held-open seek; searching status; cancel aborts"
type: feature
risk: P1
context: domain
bcps: 8
delta: MODIFIED
---

# Story e11s02 — Play form Random flow via board seek

## Context

**Before (e06, current):** kind=Random is DISABLED ("coming soon" label, taps ignored) — e06s03/e06s08 originally wired it to `create_open_challenge(mode, rated)` → `ChallengeSent("")` → InvitesScreen
(`POST /api/challenge/open`) → `ChallengeSent("")` → push InvitesScreen; the
open challenge shows as an outgoing row there.

**After (e11):** kind=Random → open the event stream
(`GET /api/stream/event`) FIRST, then start the held-open seek
(`POST /api/board/seek`). While searching, the status line shows a localized
"Searching for opponent…" (`play-status-seeking`). When the seek stream
delivers a `gameStart` event, emit `AppEvent::AiGameStarted { game_id, my_turn }`
(reused — both play.rs and chess_game.rs already attach + push
ChessGameScreen on it; `my_turn: true` placeholder, the game stream corrects
it, same as the AI path). Cancelling the search (back button, or tapping Send
again) aborts the task, which drops the seek stream → the server cancels the
seek (official API contract).

## Requirements

#### MODIFIED: `PlayScreen::send_event` for Random
**Before:** `OpponentKind::Random => AppEvent::ChallengeSent(String::new())`.
**After:** Random no longer emits `ChallengeSent` from `send_event`; the seek
task emits `AiGameStarted` on match (keep `send_event` for Friend/Computer;
Random arm removed from it and handled by `kick_seek`).

#### ADDED: `PlayScreen::kick_seek(display)`
Spawned on Send with kind=Random: opens `stream_events()` first (held for the
duration, forwarded events logged; challenge events could refresh Invites —
out of scope, just hold it open), then `seek(mode, rated)`, reads lines until
`game_id_from_seek_value` yields an id → `tx.send(AiGameStarted { .. })`.
The `tokio::task::AbortHandle` is stored in `self.seek_task: Option<AbortHandle>`;
aborting drops the stream → seek canceled server-side. Status shows
`play-status-seeking` while active; on match/failure clear it.

#### ADDED: cancel UX
- Back button while `seek_task` is active → abort task, then Pop.
- Tapping Send again while searching → abort + status "search canceled"
  (`play-status-seek-canceled`) instead of starting a second seek.
- Unit-testable seam: `fn cancel_seek(&mut self)` + `fn is_seeking(&self)`.

#### ADDED: i18n
`play-status-seeking` ("Searching for opponent…" / "Suche Gegner…"),
`play-status-seek-canceled` ("Search canceled" / "Suche abgebrochen") in
en.ftl + de.ftl.

#### MODIFIED: status keys
`send_status_key` for KIND_RANDOM → `play-status-seeking` instead of
`play-status-sending-open`.

## Implementation Steps

1. Seam + state: `seek_task: Option<AbortHandle>`, `is_seeking()`, `cancel_seek()`,
   `kick_seek` spawning event-stream-then-seek task; send_event Random arm removed;
   kick_send Random arm calls kick_seek → verify: `cd crate && cargo build --all-targets 2>&1 | tail -1 | grep -q 'Finished'`
2. Unit tests: is_seeking/cancel_seek state machine, send_status_key per kind,
   send_event no longer emits ChallengeSent for Random, AiGameStarted emitted
   for a seek game id → verify: `cd crate && cargo test --lib play 2>&1 | grep -q 'test result: ok'`
3. Touch handling: back-while-searching aborts then pops; send-while-searching
   cancels → verify: `cd crate && cargo test --lib play 2>&1 | grep -q 'test result: ok'`
4. i18n keys EN/DE → verify: `cd crate && grep -q 'play-status-seeking' i18n/en.ftl && grep -q 'play-status-seeking' i18n/de.ftl && grep -q 'play-status-seek-canceled' i18n/en.ftl && grep -q 'play-status-seek-canceled' i18n/de.ftl`
5. Gates → verify: `cd crate && cargo test 2>&1 | grep -q 'test result: ok' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 | grep -q 'Finished' && cargo fmt --check && echo GATES-OK`

## Out of scope

- Live-updating InvitesScreen from event-stream challenge events (Invites keeps
  its 5s polling)
- Auto-reconnect of a dropped seek
- e06s08's open-challenge rows in InvitesScreen stay as-is (legacy/web-created
  open challenges); Play simply no longer creates them

## Risks

- If the device network drops the idle seek connection, the seek is silently
  canceled server-side — status would keep saying "searching". Mitigation for
  later epic: watch the stream for errors and surface `ChallengeSendFailed`.
- AbortHandle abort is cooperative at await points — the seek read loop must
  await per line (it does), so abort is prompt.

## Acceptance criteria

- [ ] Random + Send starts event stream then seek; status shows searching
- [ ] gameStart → ChessGameScreen opens with the matched game id
- [ ] Back / re-tap Send aborts the task (connection dropped → seek canceled)
- [ ] Unit tests + i18n + gates green
