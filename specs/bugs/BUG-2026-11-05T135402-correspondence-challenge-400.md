# BUG-2026-11-05T135402: correspondence invites fail with 400 Bad Request

## Problem

- **Actual:** Sending a friend challenge in "Correspondence" mode fails on the
  Lichess API with `400 Bad Request`. "Unlimited" and "Real time" challenges
  work fine.
- **Expected:** Correspondence challenges are created like the other modes.
- **Reproduce:** Play screen → Correspondence → pick a days value that is not
  in Lichess's allowed set (e.g. 4, 6, 8) → Send challenge → API returns 400.

Security impact: NONE — no exploit path; this is a client-side parameter
validation mismatch.

## Root Cause Analysis

- **Code path:** PlayScreen mode `Correspondence(days)` →
  `LitcheeAdapter::send_challenge` → litchee `ChallengeRequest::days(n)` →
  `POST /api/challenge/{user}` form field `days=n`.
- **Why it fails:** Lichess only accepts a fixed set of correspondence
  day values — `1, 2, 3, 5, 7, 10, 14` (documented as
  `days: Literal[1, 2, 3, 5, 7, 10, 14]` in the official API client contract).
  The days slider allows any integer in 1..=14, so most slider positions
  (4, 6, 8, 9, 11, 12, 13) produce a `days` value the server rejects with 400.
  Verified locally: wiremock characterization shows our client sends
  `rated=false&color=random&days=N`; the field name and format are correct
  (realtime `clock.limit`/`clock.increment` work), only the value set is wrong.
- **Contributing factors:** the scope spec said "1–14 via spinner" without
  knowing Lichess restricts the value set.
- **Risk level:** Low — contained to the challenge-send path; fix is UI-side
  value snapping.

## TDD Fix Plan

1. **RED**: slider test — with stops `[1,2,3,5,7,10,14]`, taps between stops
   snap to the nearest allowed value; `value()` never returns a non-stop.
   **GREEN**: `Slider` gains optional `stops`; `tap_value`/`set_value` snap to
   the nearest stop. **verify**: `cargo test slider`
2. **RED**: PlayScreen test — selecting Correspondence always yields a mode
   whose days ∈ allowed set, regardless of tap position.
   **GREEN**: PlayScreen constructs the slider with
   `ALLOWED_CORRESPONDENCE_DAYS`. **verify**: `cargo test play_screen`
3. **RED**: wiremock test — `send_challenge` with each allowed days value
   sends exactly that `days=N` and no clock fields.
   **GREEN**: no code change expected (characterization); keeps the contract
   pinned. **verify**: `cargo test --test challenge_send`

**REFACTOR**: none needed beyond the slider change.

## Acceptance Criteria

- [ ] Correspondence invite with every selectable slider position succeeds on-device (spot-check 1, 7, 14)
- [ ] `cargo test` fully green (slider snapping, play screen, challenge_send)
- [ ] No regression: unlimited/realtime still send identical bodies as before

## Resolution

Fixed by snapping the days slider to `ALLOWED_CORRESPONDENCE_DAYS`
([1,2,3,5,7,10,14]) — `Slider::with_stops`, used by PlayScreen. Tests:
slider stop-snapping, full-track sweep asserting only allowed values,
wiremock pin that each allowed days value is sent verbatim with no clock
fields. All suites green (109 lib + integration), release build clean.
On-device spot-check of a correspondence invite still pending (needs k2).
