---
bug_id: BUG-2026-11-11T122900
date: 2026-11-11
severity: high
priority: high
scope: e06-challenge-ai
status: fixed  # e06s06 (raw POST, object-variant fixture + negative test green)
summary: >-
  "Send challenge" with opponent kind Computer fails with
  "API error: failed to decode LichessGame".
root_cause: >-
  litchee 0.2.2 `AiChallengeRequest::send()` parses the POST /api/challenge/ai
  response as `LichessGame`, whose `variant` field is `Option<LichessVariantKey>`
  (a string enum). The live Lichess API returns `"variant": {"key":"standard",
  "name":"Standard","short":"Std"}` — an object — so serde fails to decode the
  whole response (litchee-0.2.2/src/api/gameplay/games/model.rs:242 vs the live
  payload shape). Same DTO-mismatch class as /api/account/playing, which the
  adapter already works around with a raw reqwest call (get_ongoing_games).
  The existing wiremock test did not catch this because its fixture used
  `"variant":"standard"` (string) instead of the real object shape.
fix_plan: >-
  e06s06: replace the litchee call in `send_challenge_ai` with a raw reqwest POST
  to `/api/challenge/ai` (form fields level / clock.limit / clock.increment /
  days / color), parse the response as serde_json::Value and extract `id`
  (return a small local struct or just the id String). Fix the wiremock fixture
  in crate/tests/challenge_ai_open.rs to use the realistic object-shaped
  variant and assert the id is extracted. Regression test: fixture with
  variant object must succeed; missing id must error.
files:
  - crate/src/api/litchee_adapter.rs
  - crate/tests/challenge_ai_open.rs
---
