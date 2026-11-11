## Target
Epic e05-safe-inapp-update: modifies `crate/src/api/update.rs`, `crate/src/api/github.rs`, `crate/src/ui/screens/update.rs`, `kindlichess/chess_app.sh`, `.github/workflows/release.yml`, `release-please-config.json`, `flash.sh`, `flash_usb.sh`, `run-dev.sh`, plus docs.

## Dependents (fan-in)
- `crate/src/api/update.rs` — called by `crate/src/ui/screens/update.rs` (kick_update_apply)
- `crate/src/api/github.rs` — called by `crate/src/ui/screens/update.rs` (kick_update_check), `check_for_update()` public
- `crate/src/ui/screens/update.rs` — pushed from `crate/src/ui/screens/settings.rs` (check_update_button); events via `crate/src/ui/events.rs` AppEvent::Update*
- `kindlichess/chess_app.sh` — deployed by flash.sh/flash_usb.sh; launched by scriptlet/kindlichess.sh
- `.github/workflows/release.yml` — triggered on v* tags; contract with github.rs asset names
- `flash.sh` / `flash_usb.sh` — still reference removed kindle_KUAL/ tree (stale, e05s01 fixes)
- `run-dev.sh` — still references kindle_KUAL/kindlichess (stale, e05s01 fixes)

## Affected Stories
- e05s01 — restructure finish (flash scripts, run-dev.sh, docs)
- e05s02 — release pipeline (release.yml, release-please-config.json)
- e05s03 — app update client v2 (github.rs, update.rs)
- e05s04 — launcher trampoline v2 (chess_app.sh)
- e05s05 — UpdateScreen UX (update.rs screen)
- e05s06 — verification + docs
- Future e06-e10 build on e05's bundle pipeline

## Test Coverage
- crate/tests/ has api_puzzle, challenge_list, challenge_send, now_playing_decode, relations_following — NO update/github tests exist
- Gap: no unit tests for github.rs asset lookup, update.rs preflight/download/hash, chess_app.sh trampoline
- e05s03 and e05s04 add these (wiremock for GitHub API, host-harness for trampoline)

## Risk: Medium
Rationale: ~8 direct dependents across api/ui/scripts/CI, partial test coverage (no existing update/github tests), but the change is well-bounded by the spike (device tar roundtrip proven) and the staged-install design (failure leaves running install untouched). The main risk is the release asset contract change — mitigated by keeping legacy binary assets during transition.

## Recommended action
Proceed with plan-work. Add unit tests first in e05s03 (wiremock for GitHub, parser tests for df/manifest), host-harness for trampoline in e05s04. Keep legacy assets in release.yml until e05 ships.
