---
story_id: e06s07
title: "PlayScreen reorganization: kind row first, shared selector row, mode row, centered Send"
type: refactor
risk: P1
context: domain
bcps: 8
delta: MODIFIED
---

# Story e06s07 — PlayScreen layout reorganization

## Context

e06s02 appended the new controls below the existing ones (opponent y=470,
send y=600, rated y=740, kind y=860, level y=980), which reads backwards.
User-approved design (grill 2026-11-11):

1. **Row 1 (y=200, h=80):** opponent-kind group `Computer | Friend | Random`.
   The Friend button's label defaults to "Friend"; tapping it selects
   kind=Friend **and** opens the FriendlistScreen. On `FriendSelected`, the
   button label is replaced with the friend's name. The old standalone
   opponent button is removed — it assimilates into the Friend button.
   **Amended 2026-11-11 (e06 close-out):** the Random button is disabled and
   relabeled "Coming soon" (`play-kind-random-soon`, EN/DE) until e11
   (seek-based random opponent); taps are ignored and sends are blocked.
2. **Row 2 (y=327, h=96):** shared selector placeholder — exactly one of:
   AI level selector (8 × 100px buttons, kind=Computer), days selector
   (7 × 100px, mode=Correspondence and kind≠Computer), or Random info text.
   When Computer AND Correspondence are both active, the **last selected**
   one is rendered (`selector_focus`, amended 2026-11-11). The row is placed
   so the margin above it equals the margin below it (47px each, kind row
   ends 280 / mode row starts 470) — amended 2026-11-11 after the AI-level
   label was found touching the Friend button in the old 30px gap. "AI level"
   label sits **above** the row, **horizontally centered** (same centering as
   the days label) with the **same 10px margin to the row** as the days label
   has below it. The correspondence label ("Days per move") goes **below**
   the days selector. The fallback slot shows the open-challenge info text
   only for Random — the row stays **blank** when Friend is picked
   (amended 2026-11-11; the friend's name lives on the Friend kind button).
3. **Row 3 (y=470, h=80):** mode group `Unlimited | Real time 10:5 | Correspondence`
   (moved from y=200).
4. **Row 4 (y=580, h=96):** rated toggle (Casual | Rated) — the **last row of
   the upper half** (ends at 676 < 724). Only visible when usable, i.e.
   kind ≠ Computer (AI games are always casual). When hidden, the row stays
   blank so nothing below shifts.
5. Rows 1–4 must fit in the **upper half** (y < 724).
6. **Lower half:** **Send vertically+horizontally centered in the lower half**
   (center y≈1086 → button y=1038, x=(1072−600)/2=236, 600×96).
7. Status line stays at y=1240; invites/back icons unchanged.

## Requirements

#### MODIFIED: PlayScreen layout
**Before:** mode y=200, days y=310, opponent y=470, send y=600, rated y=740,
kind y=860, level y=980.
**After:** kind y=200, selector row y=327 (level | days | random info; equal
47px margins above/below, amended 2026-11-11), mode y=470, rated y=580
(last upper-half row, hidden for Computer),
send y=1038 centered in lower half. Opponent button removed; friend name
lives on the Friend kind button.

#### ADDED: Friend button opens friendlist and shows selection
Tap on kind=Friend button → push FriendlistScreen (in addition to selecting
kind). `FriendSelected(name)` → `kind_buttons[1].label = name`.

#### MODIFIED: selector labels
"AI level" label above the level row, left-aligned; "Days per move" label
below the days row (new i18n keys `play-ai-level` reused, add
`play-days-label`; EN/DE).

## Implementation Steps

1. Re-lay out constructor: new y positions, remove `opponent_button`, relabel → verify: `cd crate && cargo build 2>&1 | tail -1 | grep -q 'Finished'`
2. Selector row logic: `selector_mode()` → AiLevel | Days | RandomInfo; render + labels (AI-level label above-left, days label below) → verify: `cd crate && cargo test --lib play 2>&1 | grep -q 'test result: ok'`
3. Kind button touch: Friend selects kind AND pushes FriendlistScreen; FriendSelected updates button label → verify: `cd crate && cargo test --lib play 2>&1 | grep -q 'test result: ok'`
4. Layout invariant test: rows 1–4 all end above SCREEN_H/2 (724); send button center == lower-half center; no overlaps among all buttons → verify: `cd crate && cargo test --lib play 2>&1 | grep -q 'test result: ok'`
5. i18n EN/DE for `play-days-label`, `play-random-info` → verify: `cd crate && grep -q 'play-random-info' i18n/en.ftl && grep -q 'play-random-info' i18n/de.ftl`
6. Gates → verify: `cd crate && cargo test 2>&1 | grep -q 'test result: ok' && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 | grep -q 'Finished' && cargo fmt --check && echo GATES-OK`

## Out of scope

- Computer + Correspondence simultaneously shows the last-selected selector
  (amended 2026-11-11); the non-rendered one stays unselectable in that
  combination — hit-testing is routed by `selector_mode()` so the hidden
  group can never shadow the visible one (BUG-2026-11-11T153246)
- e07's reusable list-picker modal

## Risks

- Mode row moving down pushes nothing else (send/rated were already below) —
  layout test catches regressions
- Friend name can be long — button label font may need shrinking for long
  names (clamp font size by label length)

## Acceptance criteria

- [ ] Kind row is the first row; friend name shown on the Friend button
- [ ] Selector row hosts exactly one of level/days/info
- [ ] Mode row below selector row; rated toggle is the last upper-half row (visible only when kind ≠ Computer)
- [ ] Send centered in lower half
- [ ] Layout + state unit tests green; full gates green
