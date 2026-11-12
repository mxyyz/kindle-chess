---
bug_id: BUG-2026-11-11T123000
date: 2026-11-11
severity: medium
priority: high
scope: e06-open-challenges
status: fixed  # e06s08 (ChallengeSummary.open, invites-open-challenge label, wiremock destUser:null + cancel green)
summary: >-
  Open challenges (opponent kind Random, POST /api/challenge/open) do not show
  up as usable rows in the InvitesScreen outgoing section — they render with an
  empty opponent label `" (created)"` because `map_challenge` maps
  `destUser: null` to `opponent: ""`, and there is no marker that the row is an
  open challenge. Cancel from the outgoing row must work for them.
root_cause: >-
  `create_open_challenge` maps the `LichessOpenChallenge` response with
  `opponent: String::new()` and no open flag; `list_challenges` maps outgoing
  entries with `dest_user: None` to the same empty string. The InvitesScreen
  label is `format!("{} ({})", c.opponent, c.status)` so the row looks blank.
  (Decode itself works: litchee `LichessChallenge.dest_user` is Option and all
  other fields have serde defaults — to be confirmed by a wiremock test with a
  realistic `"destUser": null` entry.)
fix_plan: >-
  e06s08: add `open: bool` to `ChallengeSummary`; set it in
  `create_open_challenge` and in `map_challenge` when `dest_user` is None on an
  outgoing entry. InvitesScreen outgoing label for open challenges: localized
  `invites-open-challenge` ("Open (random)") instead of the empty opponent.
  Cancel already routes to `cancel_challenge(id)` — verify with a wiremock test
  that POST /api/challenge/{id}/cancel is sent for an open challenge id.
files:
  - crate/src/models/board_api.rs
  - crate/src/api/litchee_adapter.rs
  - crate/src/ui/screens/invites.rs
  - crate/i18n/en.ftl
  - crate/i18n/de.ftl
  - crate/tests/challenge_list.rs
---
