---
story_id: e07s08
title: "ListPicker tap-to-select mode (no Select button) — used for friend-opponent picker"
type: feature
risk: P2
context: domain
bcps: 3
delta: MODIFIED
---

# Story e07s08 — tap-to-select ListPicker variant

## Context

**Before (e07s01):** ListPicker requires tapping an item then tapping "Select" to
confirm. For the friend-opponent picker this is two taps; the user wants one tap
on a friend to immediately select and close.

**After:** Add a `tap_to_select: bool` mode to ListPicker. When true, tapping an
item immediately returns `PickResult::Selected(idx)` (no Select button drawn).
The friend-opponent picker in PlayScreen uses this mode.

## Requirements

#### MODIFIED: `ListPicker` gets a `tap_to_select: bool` field
- When `true`: `handle_touch` on an item row returns `Selected(item_idx)` directly.
- When `true`: the Select button is not drawn (Cancel stays).
- When `false` (default): current behavior preserved (highlight + Select confirm).

#### MODIFIED: `PlayScreen::open_friend_picker` / `FriendsLoaded`
- Set `picker.tap_to_select = true` so the friend picker uses tap-to-select.

#### Unit tests
- tap_to_select=true: touch on item → `Selected(idx)` immediately.
- tap_to_select=false: touch on item → `NotHandled` (highlight only), Select → `Selected`.
- Select button not drawn when tap_to_select (verify via draw call count or rect check).

## Implementation Steps

1. Add `tap_to_select` field + branch in `handle_touch` and `draw` → verify: `cargo test --lib list_picker`
2. Set `tap_to_select = true` in PlayScreen picker creation → verify: `cargo build` + `grep tap_to_select play.rs`
3. Gates → verify: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

## Acceptance criteria

- [ ] Friend picker: one tap on a friend selects and closes the modal
- [ ] Select button hidden in tap-to-select mode
- [ ] Existing ListPicker behavior unchanged when tap_to_select=false
- [ ] Gates green
