# BUG-2026-11-09T200000: run-dev.sh black screen after e04s04 layout constants

## Problem

run-dev.sh shows a black screen instead of the home screen. Introduced by
e04s04 (layout constants), possibly present since e04s03 (screen split) —
not tested in between.

## Root Cause Analysis

In `ui/renderer.rs` `create_window()`, the X11 window height argument was
replaced with literal `0` instead of `SCREEN_H_U16` during the automated
1072/1448 → layout-constant substitution in e04s04. The window is created
with height 0, so nothing renders (black screen).

The mangled replacement: original `1072,\n 1448,` became
`SCREEN_W_U16,\n 0,` — the second number was lost in the multi-line
sed replacement.

Risk level: Medium (visual regression, no data loss).

## TDD Fix Plan

1. **RED**: Window height is 0 → black screen (manual visual check via run-dev.sh)
   **GREEN**: Fix line 439 in renderer.rs: `0` → `SCREEN_H_U16`
   **verify**: `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`

## Acceptance Criteria

- [x] Window created with correct height (SCREEN_H_U16)
- [x] cargo build green
- [x] cargo clippy -D warnings green
- [x] All 196 tests pass
- [ ] run-dev.sh shows home screen (manual visual confirmation needed)

## Resolution

Fixed in renderer.rs create_window: height arg 0 → SCREEN_H_U16.
