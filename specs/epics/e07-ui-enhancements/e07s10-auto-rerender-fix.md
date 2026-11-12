---
story_id: e07s10
title: "Fix auto-rerender: periodic Tick drives polls on OngoingGames/Invites + Home puzzle badge fetch"
type: bug
risk: P1
context: domain
bcps: 5
delta: MODIFIED
---

# Story e07s10 — auto-rerender fix for polling + Home puzzle badge

## Context

**Bug (S05):** The poll check in OngoingGames/Invites `render()` only fires when
`render()` is called. After bootstrap (first 1.5s), the runner's `recv_timeout`
loop only calls `render()` on events (touch, expose, etc.) — NOT on a timer.
So polls don't fire automatically; the user must switch pages to trigger a
render that picks up new data.

Also: the Home puzzle badge (`puzzle_badge`) is never set to true because no
code fetches the daily puzzle status on the Home screen.

**After:** A periodic `AppEvent::Tick` is sent by the runner (e.g. every 1s),
which triggers a render on the active screen. The poll logic in render() then
fires correctly. Home screen kicks a daily-puzzle-status fetch on first render
and sets `puzzle_badge = true` when the daily is available.

## Requirements

#### MODIFIED: `app/runner.rs` — send periodic Tick after bootstrap
- After `x11_alive` is true (or after bootstrap deadline), on each
  `recv_timeout` timeout, also send `AppEvent::Tick(Duration)` to the event
  channel (or handle it inline by calling render).
- Actually simpler: on timeout, if `x11_alive`, call `screen.render()` directly
  (like the bootstrap path but without the deadline limit). This drives the
  poll check in render().

#### MODIFIED: `ongoing_games.rs` + `invites.rs` — poll works via periodic render
- The existing poll logic in render() now fires correctly because render is
  called periodically. No change needed to the poll logic itself, just verify
  it works.
- But: after `OngoingGamesLoaded`, `set_page(self.page_index)` is called —
  if the new game is on a different page, the user won't see it. Fix: on new
  data, if the current page is empty but data exists, jump to page 0.

#### MODIFIED: `home.rs` — fetch daily puzzle status for badge
- On first render (auth_started path), if app is online, kick a fetch for
  the daily puzzle. When it arrives (new AppEvent or reused PuzzleLoaded),
  check if it's the daily and set `puzzle_badge = true`.
- Simpler: just fetch daily puzzle and set badge=true (daily always exists).
  The "unsolved" check can be deferred.

## Implementation Steps

1. runner.rs: on timeout after bootstrap, call render on active screen → verify: `cargo build`
2. ongoing_games.rs: on OngoingGamesLoaded, if current page has no items, reset to page 0 → verify: `cargo test --lib ongoing_games`
3. home.rs: kick daily puzzle fetch on first render when online, set puzzle_badge on arrival → verify: `cargo build` + `grep puzzle_badge home.rs`
4. Unit tests + gates

## Acceptance criteria

- [ ] OngoingGames auto-refreshes without page switching (poll fires via periodic render)
- [ ] New games appear automatically within 5s
- [ ] Home puzzle button shows badge when daily puzzle is available
- [ ] Gates green
