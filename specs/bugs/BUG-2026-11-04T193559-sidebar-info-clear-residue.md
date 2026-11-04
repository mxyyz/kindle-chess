# BUG-2026-11-04T193559: Solved-puzzle info block leaves glyph residue on "Next puzzle"

## Problem

- **Actual:** After a puzzle is solved, the sidebar's info block (puzzle
  rating + up to 4 themes) is drawn centered across the full 1072 px sidebar
  width. When the next puzzle loads (or is requested), the block is cleared by
  painting a white rectangle over a fixed **600 px** centered band. Themes text
  wider than 600 px (4 themes joined with " · " at 24 px font easily exceeds
  it) keeps its outer glyphs — visible residue to the left and right of the
  clear rectangle. The info is also only cleared once the *next* puzzle
  finishes loading, not at button-press time.
- **Expected:** Pressing "Next puzzle" (or applying new settings) clears the
  rating/themes block completely, leaving no residue regardless of text width.
- **Reproduce:** Solve a puzzle with long theme names, press "Next puzzle",
  watch the sidebar: theme text remnants survive outside the 600 px band.

Security impact: NONE — pure rendering bug, no data exposure.

## Root Cause Analysis

Verified by reading the sidebar widget's render path:

- The info block is cleared with `draw_rectangle(self.info_band, White)` where
  `info_band` is a hard-coded 600 px centered rect, while `draw_info_block`
  centers the themes line across the whole sidebar width (`area.width`). Any
  text wider than 600 px overflows the clear rect on both sides — the residue
  the user sees. Widening the band is not a fix: the stacked hint button at the
  right edge (134 px icon + margin, vertically overlapping the info band)
  caps the safe clear width below 952 px, and worst-case theme strings can
  exceed even that.
- Secondary issue: `PuzzleScreen`'s `NextPuzzle` / `ApplyPuzzleParams`
  handlers only call `set_message("Loading…")`; the info block is cleared
  later, when the new `PuzzleLoaded` event arrives with status `Solving`.

Risk level: Low — rendering only; worst case is stale pixels until the next
full repaint.

## TDD Fix Plan

1. **RED**: unit test in the sidebar widget: after the first full paint, an
   *info change* (`set_puzzle_info` with different rating/themes, e.g. solved →
   cleared) must mark the widget for a full repaint (`needs_full_repaint`),
   while a *status-only* change (`set_status`) must not — the cheap band path
   stays for per-move updates. Also: setting identical info must not trigger a
   repaint.
   **GREEN**: in `set_puzzle_info`, when the new `(rating, themes)` differs
   from the current values, set `needs_full_repaint = true` — the full repaint
   whites the whole sidebar area, so no residue is possible regardless of text
   width.
   **verify**: `cargo test --lib puzzle_sidebar`

2. **RED**: (wiring, verified by build + suite) — no unit seam exists for the
   screen handler; behavior is "info cleared at Next-press time", covered
   indirectly by cycle 1's predicate.
   **GREEN**: in `PuzzleScreen`'s `NextPuzzle` and `ApplyPuzzleParams`
   handlers, call `sidebar.set_puzzle_info(0, &[], &PuzzleStatus::Solving)`
   next to `set_message("Loading…")` so the block clears immediately on press.
   **verify**: `cargo test` (full suite) + `cargo build`

**REFACTOR**: none anticipated; the 600 px bands stay for the status line
(short strings only).

## Acceptance Criteria

- [ ] "Next puzzle" clears rating/themes immediately at press, with zero glyph
      residue for any theme-string width (full sidebar repaint).
- [ ] Status-only updates still use the cheap band diff (no full repaint).
- [ ] All new + existing tests pass; build and fmt clean.

## Resolution

<!-- filled in by validate-fix -->
