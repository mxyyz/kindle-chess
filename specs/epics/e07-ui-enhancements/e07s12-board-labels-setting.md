---
story_id: e07s12
title: "Board file/rank labels visibility tied to persistent AppConfig setting (default on)"
type: feature
risk: P2
context: domain
bcps: 3
delta: MODIFIED
---

# Story e07s12 — board labels visibility via persistent setting

## Context

**Before (e07s07):** `draw_labels()` always draws file/rank labels on the board
edges. The user wants this controlled by a persistent setting.

**After:** Add `board_labels: bool` to `AppConfig` (default `true`). The board
widget reads this config (passed via a setter or constructor param) and only
draws labels when true. The setting UI will be added in e08-settings-tabs.

## Requirements

#### ADDED: `AppConfig.board_labels: bool` (default true)
- In `utils/config.rs`, add field with `#[serde(default = "default_true")]`
  or just default in `Default` impl.

#### MODIFIED: `BoardWidget` — label visibility flag
- Add `show_labels: bool` field (default true), with `set_show_labels(bool)`.
- `draw_labels()` checks the flag and returns early if false.
- The board widget gets the flag from config when created/updated (the chess_game
  screen passes `config.read().board_labels` to the board widget).

#### Unit tests
- draw_labels skipped when show_labels=false (can't test draw directly, but
  test the flag logic).
- AppConfig default has board_labels=true.

## Implementation Steps

1. config.rs: add `board_labels: bool` field, default true → verify: `cargo test --lib config`
2. board.rs: add `show_labels` field + setter, guard draw_labels → verify: `cargo test --lib board`
3. chess_game.rs (and puzzle.rs): pass config value to board widget → verify: `cargo build`
4. Gates → verify: full test + clippy + fmt

## Out of scope

- Settings UI to toggle the setting (e08-settings-tabs)

## Acceptance criteria

- [ ] `AppConfig.board_labels` exists, defaults true
- [ ] Board labels hidden when set to false
- [ ] Gates green
