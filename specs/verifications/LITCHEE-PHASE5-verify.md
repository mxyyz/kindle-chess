# litchee migration — Phase 5 (Testing) verification evidence

Source plan: `specs/LITCHEE_MIGRATION_PLAN.md` §4 Phase 5
Branch: `api-migration` (not `main`)
Verified at: 2026-10-29T22:38:03Z
Verifier: automated run (`cargo fmt --check && cargo test`)

## 1. Unit tests — `tests/api_puzzle.rs` through the adapter ✅

Added 8 adapter-driven tests exercising the production path (`LitcheeAdapter` →
litchee `LichessClient` → wiremock mock server → DTO decode → app-type mapping):

- `adapter_daily_puzzle_parses_and_maps_from_mock_server`
- `adapter_puzzle_by_id_hits_the_id_path`
- `adapter_next_puzzle_refetches_the_full_record` (proves the id-only `/next`
  response triggers the full `/puzzle/{id}` re-fetch)
- `adapter_maps_missing_optionals_to_defaults` (litchee `Option<>` → `0`/`None`)
- `adapter_daily_puzzle_errors_on_server_error` (500 → `Err`)
- `adapter_constructed_with_mock_base_url_sends_bearer_token`
- `adapter_home_puzzle_returns_daily_when_daily_not_played` (NDJSON activity)
- `adapter_home_puzzle_returns_next_when_daily_already_played`

The existing `PuzzleFetcher`-seam tests are retained (still valid for the
`*_with` parse helpers).

## 2. Integration — adapter with mock base URL ✅

New `LitcheeAdapter::with_base_url(token, base_url)` constructor (documented as
the test/mock hook; uses `LichessClientBuilder::base_url`). The bearer-token
test proves an authenticated adapter built against a mock base URL sends
`Authorization: Bearer …` and decodes the response.

Dev-dependency added: `wiremock = "0.6"` (same version litchee's own test suite
pins).

## 3. Manual test on dev machine — ⏳ USER ACTION REQUIRED

`./run-dev.sh` launches a GUI (Xephyr/x11vnc/feh) and the auth flow needs a
phone to scan the QR. Cannot be exercised headlessly by the agent. Remaining
manual UAT checklist: auth flow, puzzle fetch, game play.

## 4. Cross-compile armv7 — ✅ PASSED

Command (note: agent sandbox `LD_LIBRARY_PATH` must be unset for podman/crun):

```bash
env -u LD_LIBRARY_PATH bash -lc \
  'cd kindle_chess && RUSTFLAGS="-C target-feature=+crt-static" \
   cross build --target armv7-unknown-linux-musleabi --release'
```

Result: `Finished release profile … in 4.20s` (incremental on Phase 0 cache).
Artifact: `target/armv7-unknown-linux-musleabi/release/kindle-hello`
— ELF 32-bit LSB, ARM EABI5, statically linked, 13 MB (unstripped).
The `aws-lc-sys` risk from §6 did not materialize.

## 5. Deploy to Kindle — ⏳ BLOCKED

`./flash.sh` (scp to root@192.168.15.244): device unreachable at verification
time (`ping` 100% packet loss). Re-run when the Kindle is on the LAN.

## Terminal verdict

- Command: `cargo fmt --check && cargo test` (single contiguous run)
- Exit code: 0
- Result: **97 passed, 0 failed** (76 unit + 15 api_puzzle + 3 lib + 3
  now_playing doc/decode tests)
- clippy: no new warnings (only pre-existing `module has the same name as its
  containing module` for `api::api`)

## Gaps

- Manual dev-machine UAT (item 3) and on-device deploy (item 5) remain open;
  both need a human / the physical device.
