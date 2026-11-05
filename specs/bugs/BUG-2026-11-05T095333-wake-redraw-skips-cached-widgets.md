# BUG-2026-11-05T095333: wake-after-lock redraw skips cached widgets (board/sidebar stay invisible)

## Problem

After unlocking the Kindle (powerd `outOfScreenSaver` → `AppEvent::DeviceWake`),
the game screen is not repainted: the board and sidebar widgets stay invisible
(the screensaver/logo remains). The app is still responsive — touching a square
makes the selection indicator appear, i.e. widgets repaint when individually
invalidated by interaction, but the wake redraw itself paints nothing.

Expected: on unlock, the top screen is fully redrawn (board + sidebar visible
again), and the game stream refetches the latest state.

Repro: start a game → lock the device (screensaver/logo shows) → unlock →
board/sidebar missing until blindly tapped.

Security impact: NONE — no security exploit path identified (local UI rendering only).

## Root Cause Analysis

The screensaver drawn on `DeviceLock` paints over the whole framebuffer, but
the partial-render widgets keep their "already painted" caches:

- The board widget diffs its current position against a last-drawn snapshot and
  only repaints when `force_full_repaint` is set (via `invalidate()`); the
  sidebar widget likewise uses a full-repaint flag plus drawn-content snapshots.
- The wake path in the app event loop called `screen.render(...)` directly.
  The `Screen::on_reveal()` hook — which invalidates exactly those widgets —
  was only invoked on `Pop`, never on `DeviceWake`.
- So the wake redraw rendered "nothing changed" for the cached widgets and the
  screensaver stayed on screen. Widgets reappeared only when a touch triggered
  `invalidate()` on them.

Verified by code inspection of the render/invalidate paths and the device
symptom (interaction-triggered repaints work, wake redraw does not).
Contributing factor: the e02 screensaver feature (t08) introduced drawing over
the app window without going through the screen stack, which the existing
invalidate-on-reveal mechanism didn't cover for the lock/wake cycle.

Risk level: Low (visual staleness only; input and game logic unaffected).

## TDD Fix Plan

1. **RED**: widget-level contract tests — `BoardWidget::invalidate()` must set
   the force-full-repaint flag; `SidebarWidget::invalidate()` must set its
   full-repaint flag. (Fails before the flags exist/are set — guards the
   contract the fix relies on.)
   **GREEN**: tests assert the flags flip after `invalidate()` — already
   satisfied by existing widget code; tests added as regression net.
   **verify**: `cargo test --lib invalidate_forces_full_repaint`

2. **RED**: wake path must invalidate the top screen before rendering —
   observable as: after DeviceWake, `on_reveal()` has been called on the top
   screen (board+sidebar invalidated), so the render is a full repaint.
   **GREEN**: in the app loop's `DeviceWake` handler, call
   `screen.on_reveal()` before `screen.render(...)`.
   **verify**: `cargo build && cargo test` + on-device UAT (t07): lock → logo
   screensaver → unlock → board+sidebar fully visible without touching.

**REFACTOR**: none needed — the fix reuses the existing `on_reveal` hook.

## Acceptance Criteria

- [x] Widget contract tests for `invalidate()` pass (`cargo test --lib`)
- [x] Wake handler calls `on_reveal()` before rendering the top screen
- [x] `cargo build` + full `cargo test` green
- [ ] On-device UAT: unlock after lock fully repaints board+sidebar (no blind
      tapping needed) — part of e02 t07

## Resolution

Fixed on branch `e02-native-kindle-integration` (commit dc898bf): app-loop
`DeviceWake` handler now calls `screen.on_reveal()` before the redraw; widget-level
regression tests added for the invalidate contract. Confirmed on-device by the
reporter: unlock after lock now fully repaints board+sidebar. Remaining e02 step:
t07 UAT log, then merge to main.
