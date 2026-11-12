---
bug_id: BUG-2026-11-11T153246
date: 2026-11-11
severity: medium
priority: high
scope: e06s07-play-selector-row
status: fixed  # selector_touch routes by selector_mode(); RED->GREEN verified, gates green
summary: >-
  AI-level selector needs multiple presses / some levels unselectable: the
  hidden days group shares the selector row and is hit-tested first, so it
  swallows most taps meant for the level buttons.
files:
  - crate/src/ui/screens/play.rs
---

# BUG-2026-11-11T153246 — AI-level taps swallowed by the hidden days selector

## Problem

**Actual:** with kind = Computer, tapping an AI-level button sometimes does
nothing (needs repeated presses) and some levels seem unselectable — no
obvious pattern, depends on exact tap position.
**Expected:** one tap on a visible level button selects that level.

## Root Cause Analysis

1. **Reproduce:** Play → Computer → tap level buttons. Taps with
   x ∈ [138, 934) on the selector row never change the level; only the slivers
   x ∈ [94, 138) and x ∈ [934, 978) work.
2. **Isolate:** the Play screen touch handler hit-tests `days_buttons` before
   `level_buttons`, unconditionally. Both groups live on the same selector
   row (y=310, h=96): days span x ∈ [138, 934), levels span x ∈ [94, 978) —
   they overlap almost completely. A tap inside a days rect sets
   `selected_days` and returns early, so the level loop is never reached.
3. **Hypothesis (confirmed):** the hidden selector group shadows the visible
   one. Fix: hit-test only the group that `selector_mode()` currently renders.
4. **Verify:** RED unit test taps the center of every level button in Computer
   mode and asserts `ai_level` follows; fails with the days-first order, passes
   once routing is by `selector_mode()`.

Security impact: NONE — local UI hit-testing bug.

## Same-run adjustments (user request 2026-11-11)

- **Last-selected wins:** when Computer AND Correspondence are both active,
  the selector row renders the one selected last (new `selector_focus` state:
  Computer tap → Level, Correspondence tap / days tap → Days). Previously the
  level row always won.
- **AI-level label:** centered horizontally like the days label, vertically
  centered in the gap above the selector row (still above, balanced margins).

## TDD Fix Plan

1. **RED**: move the days+level touch loops into `selector_touch(x, y)`
   unchanged (days first); test `every level button selectable in Computer
   mode` → fails.
   verify: `cd crate && cargo test --lib play 2>&1 | grep -q FAILED`
2. **GREEN**: `selector_touch` routes by `selector_mode()` — only the visible
   group is hit-tested. Add `selector_focus` + last-selected-wins in
   `selector_mode()`; set focus on Computer/Correspondence/days taps.
   verify: `cd crate && cargo test --lib play 2>&1 | grep -q 'test result: ok'`
3. **GREEN (label)**: center the AI-level label (horizontally centered,
   vertically centered in the gap above the row).
   verify: layout tests + `cargo test --lib play`
4. **Gates**: `cd crate && cargo test 2>&1 | grep -q 'test result: ok' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 | grep -q 'Finished' && cargo fmt --check && echo GATES-OK`

## Acceptance Criteria

- [ ] One tap on any level button in Computer mode selects that level (unit test)
- [ ] Days taps in correspondence mode still work; neither group shadows the other
- [ ] Computer + Correspondence renders the last-selected selector
- [ ] AI-level label centered above the row like the days label is centered below
- [ ] All gates green

## Resolution

Fixed 2026-11-11 on branch e06-play-form-enhancements:
- Selector-row hit-testing extracted into `selector_touch(x, y)`, which routes
  by `selector_mode()` — only the currently rendered group (level | days |
  info) is hit-tested, so the hidden group can no longer shadow the visible
  one. RED test (tap center of each level button in Computer mode) failed with
  the old days-first order (`ai_level` stuck at 3), passes now for all 8.
- Same run, user adjustments: `selector_focus` (Level | Days) makes the shared
  row render the LAST selected selector when Computer + Correspondence are both
  active (set by `select_kind(Computer)` / `select_mode(Correspondence)` /
  days taps); the "AI level" label is horizontally centered above the row,
  vertically centered in the gap (days label stays centered below).
- Tests: `every_level_button_selectable_with_one_tap_in_computer_mode`,
  `every_days_button_selectable_in_correspondence_mode`,
  `computer_and_correspondence_show_last_selected_selector`, layout invariants
  extended to both focus variants. Full gates green.
