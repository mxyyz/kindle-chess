---
story_id: e07s11
title: "OngoingGames square cards with title+detail rows; Invites icon buttons (check/close) + wider labels"
type: feature
risk: P1
context: domain
bcps: 8
delta: MODIFIED
---

# Story e07s11 — layout overhaul: OngoingGames squares + Invites icon buttons

## Context

**Bug (S06):** Detail info (rating, speed, etc. from e07s06) overlaps the button
boxes because everything is crammed into a single-line Button label.

**OngoingGames fix:** Each game becomes a square card (width × width). The
"vs {opponent}" is the card title (larger font). Below it, additional info
(rating, speed, rated) each on its own row in smaller font.

**Invites fix:** Replace "Accept"/"Decline" text buttons with IconButtons using
`check.png` / `close.png` (already in assets). This frees horizontal space —
increase the name label width (NAME_W) to use the gained space.

## Requirements

#### MODIFIED: `ongoing_games.rs` — square cards
- Each game card: square (e.g. 380×380), arranged 2×2 grid.
- Title: "vs {opponent}" at top of card (28px).
- Detail rows below title, each on own line, 20px font:
  - Rating: "{rating}"
  - Speed: "{speed}"
  - Rated badge if rated
  - My-turn indicator "> <"
- Replace Button-based rows with custom card rendering (struct GameCard with
  rect + title + detail_lines).

#### MODIFIED: `invites.rs` — icon buttons for accept/decline
- incoming_accept: [IconButton; 4] using "check.png"
- incoming_decline: [IconButton; 4] using "close.png"
- outgoing_cancel stays as Button (or also becomes icon — check.png? No, keep
  text "Cancel" for outgoing since there's space).
- NAME_W increased from 420 to ~700 (gained from removing 2×200px text buttons,
  replaced by 2×134px icon buttons = saves ~132px per side).

#### Unit tests
- Card layout: 2×2 grid positions don't overlap.
- Invites: icon button rects don't overlap name rects.
- Label width: NAME_W > old value.

## Implementation Steps

1. ongoing_games.rs: GameCard struct + 2×2 square layout + detail row rendering → verify: build + test
2. invites.rs: IconButton arrays for accept/decline + NAME_W increase → verify: build + test
3. Touch handling updated for new rects → verify: build
4. Gates → verify: full test + clippy + fmt

## Acceptance criteria

- [ ] OngoingGames: each game is a square card with title + detail rows, no overlap
- [ ] Invites: accept= check.png icon, decline = close.png icon, labels wider
- [ ] No rect overlaps in either screen
- [ ] Gates green
