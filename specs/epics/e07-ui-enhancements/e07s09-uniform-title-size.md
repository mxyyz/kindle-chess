---
story_id: e07s09
title: "Uniform title font size (56px everywhere) + rename Play title to New Game / Neue Partie"
type: feature
risk: P2
context: domain
bcps: 3
delta: MODIFIED
---

# Story e07s09 — uniform title size + Play title rename

## Context

**Before (e07s02):** Two title helpers exist — `draw_title_bar` (36px, used by
play, ongoing_games, friendlist, invites, chess_auth) and `draw_page_title`
(56px, used by settings, game_actions, puzzle_settings, update). The user wants
the Settings-page size (56px) everywhere.

Also the Play screen title is "Play vs friend" / "Gegen Freund spielen" — user
wants it renamed to "New Game" / "Neue Partie".

**After:** Single `draw_title_bar` at 56px (PAGE_TITLE_FONT) used by ALL screens.
`draw_page_title` removed or aliased. Play title i18n key changed.

## Requirements

#### MODIFIED: `layout.rs` — unify to 56px
- `TITLE_FONT` → 56.0 (or remove `draw_title_bar`/`draw_page_title` split and
  keep one function at 56px).
- All 9 screens use the same helper at 56px.

#### MODIFIED: i18n — play-title
- en.ftl: `play-title = New Game`
- de.ftl: `play-title = Neue Partie`

#### Unit tests
- layout test: single title font constant = 56.0.
- i18n parity test already exists — just verify keys match.

## Implementation Steps

1. layout.rs: set TITLE_FONT = 56.0, make draw_page_title an alias or merge → verify: `cargo test --lib layout`
2. Update all screens to use the unified helper (they already call draw_title_bar/draw_page_title — just ensure both use 56px) → verify: `cargo build`
3. i18n: rename play-title EN/DE → verify: `grep 'play-title' en.ftl de.ftl`
4. Gates → verify: full test + clippy + fmt

## Acceptance criteria

- [ ] All screens render titles at 56px
- [ ] Play screen title shows "New Game" / "Neue Partie"
- [ ] Gates green
