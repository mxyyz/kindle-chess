---
story_id: e06s02
title: "PlayScreen UI: rated/casual toggle + opponent picker (Friend/Computer/Random)"
type: feat
risk: P0
context: domain
bcps: 8
delta: MODIFIED
---

# Story e06s02 — PlayScreen UI

## Context

`PlayScreen` (`crate/src/ui/screens/play.rs`, 465 lines) currently has: three
mode buttons (unlimited / real-time 10:5 / correspondence), a 7-button days
group shown only in correspondence mode, an opponent button that pushes
`FriendlistScreen` (result returns as `AppEvent::FriendSelected`), a send
button, and a status line. Canvas is 1072x1448 with constants in
`ui/layout.rs`.

**Dependency decision:** the sliced story mentioned a list-picker modal "from
e07", but e07 lands after e06. Friend picking therefore stays on the existing
`FriendlistScreen` flow — no cross-epic dependency.

**Zoom-out:** `play.rs` owns the challenge form; callers: `home.rs` pushes it,
`friendlist.rs` returns `FriendSelected`, `runner`/events route
`ChallengeSent`/`ChallengeSendFailed`. Contract: one `handle_event` match on
`AppEvent`, buttons are `widgets::Button` with rects inside
`layout::SCREEN_W/H`.

## Requirements

#### MODIFIED: Opponent selection becomes kind-based
**Before:** opponent button opens FriendlistScreen; only friends can be
challenged; send is always casual.
**After:** an opponent-kind selector offers Friend / Computer / Random.
Friend opens FriendlistScreen as today. Computer shows a level selector
(1–8). Random needs no opponent (open challenge). The send flow (e06s03)
routes per kind.

#### ADDED: Rated/casual toggle
A two-button group (Rated / Casual) above the mode row; default Casual.
Applies to Friend and Random challenges; hidden/disabled for Computer
(Lichess AI games are always casual — no `rated` on `AiChallengeRequest`).

#### ADDED: EN/DE i18n for all new strings
New keys in `crate/i18n/en.ftl` and `crate/i18n/de.ftl`:
`play-rated`, `play-casual`, `play-kind-friend`, `play-kind-computer`,
`play-kind-random`, `play-ai-level`.

## Implementation Steps

1. Add `rated_toggle: [Button; 2]` and `kind_buttons: [Button; 3]` +
   `level_buttons`/spinner for AI level to `PlayScreen`; wire into
   `handle_event` touch handling → verify: `cd crate && cargo build 2>&1 | tail -1 | grep -q 'Finished'`
2. Layout: all new buttons within `layout::SCREEN_W/H` (1072x1448), no overlap
   with mode/days/opponent/send rows; add a unit test asserting every new
   button rect is inside the screen and non-overlapping → verify: `cd crate && cargo test --lib play 2>&1 | grep -q 'test result: ok'`
3. Add EN/DE fluent keys to `crate/i18n/en.ftl` + `de.ftl`; use them in button
   labels → verify: `cd crate && grep -q 'play-rated' i18n/en.ftl && grep -q 'play-rated' i18n/de.ftl && grep -q 'play-kind-computer' i18n/en.ftl && grep -q 'play-kind-computer' i18n/de.ftl`
4. Hit-test unit tests for the new buttons (tap inside rect toggles state) → verify: `cd crate && cargo test --lib play 2>&1 | grep -q 'test result: ok'`

## Verification Script (Step-by-Step)

1. `./run-dev.sh` → open Play: rated/casual toggle visible, three opponent
   kinds visible, AI level selector appears when Computer selected
2. `cd crate && cargo test --lib play` — layout + hit-test tests green

## Out of scope

- The reusable list-picker modal (that's e07s01; e06 uses FriendlistScreen)
- Send-flow routing (e06s03)
- Variants (deferred)

## Risks

- Screen is already dense (mode + days + opponent + send rows): new rows must
  not push buttons off the 1448px canvas — caught by the layout unit test
- Triple-tap quit gesture must still work — buttons use existing TouchKind::Up
  path

## Acceptance criteria

- [ ] Rated/casual toggle rendered and tappable
- [ ] Opponent-kind selector rendered; Computer reveals level 1–8
- [ ] All new labels have EN + DE strings
- [ ] Layout unit test proves in-bounds, non-overlapping rects
- [ ] `cargo test --lib play` green
