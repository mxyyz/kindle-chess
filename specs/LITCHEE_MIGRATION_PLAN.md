# litchee Migration Plan: Replacing kindle_chess/src/api with litchee

## Executive Summary

**Verdict: litchee is viable for the Kindle target, with caveats.**

litchee 0.2.1 covers every endpoint the app uses (OAuth PKCE, account profile, board game play/stream, puzzles, ongoing games, event stream). It is MIT-licensed, actively maintained (last release 2026-10-17), and listed in the official Lichess API docs as a Rust client.

The main risks:
1. **reqwest 0.12 -> 0.13 upgrade**: litchee depends on `reqwest ^0.13.4` with the `rustls` feature. reqwest 0.13 switched its rustls backend from `ring` to `aws-lc-rs`, which pulls in `aws-lc-sys` — a C/C++ build dependency that requires `cmake`, a C compiler, and can be problematic for cross-compilation to `armv7-unknown-linux-musleabi`. This needs a spike test.
2. **Binary size increase**: litchee adds ~15 transitive dependencies. On a Kindle with limited storage, this could matter.
3. **DTO shape differences**: litchee's types (`LichessPuzzle`, `LichessGameFull`, `LichessNowPlayingGame`, etc.) have different field names and optionality than the app's hand-rolled types. A mapping/adapter layer is needed.
4. **OAuth flow differences**: litchee provides the URL-building and code-exchange primitives but does NOT include a callback server. The app's axum-based callback server stays; only the PKCE/token-exchange logic moves to litchee.

---

## 1. Current API Surface (what needs replacing)

### Files in `kindle_chess/src/api/`:

| File | Lines | Purpose | litchee replacement |
|------|-------|---------|---------------------|
| `api.rs` | 201 | Puzzle endpoints: `get_daily_puzzle`, `get_puzzle_by_id`, `get_next_puzzle`, `get_home_puzzle`, `daily_already_played` | `client.puzzles().daily()`, `.get(id)`, `.next(angle, difficulty, color)`, `.activity(max, ...)` |
| `oauth.rs` | 456 | OAuth2 PKCE flow: `OAuth2Client`, `start_auth_flow`, `exchange_code`, `run_auth_server` (axum callback), `get_user_info`, `authenticate`, `load_token`, `authenticated_request`, `logout` | `client.oauth().authorization_url()`, `.exchange_code()`. Callback server stays (axum). Token persistence stays. `client.account().profile()` for user info. |
| `board.rs` | 387 | Board game API: `get_ongoing_games`, `stream_event`, `move_piece`, `resign_game`, `abort_game`, `stream_game_event`, `handle_game_event` | `client.games().now_playing(nb)`, `client.board().stream_events()`, `client.board().make_move()`, `.resign()`, `.abort()`, `client.board().stream_game()` |
| `github.rs` | 144 | GitHub Releases client (update checker) — NOT lichess, stays as-is | No change |
| `update.rs` | 123 | In-app updater (downloads binary from GitHub) — NOT lichess, stays as-is | No change |
| `traits.rs` | 58 | `PuzzleFetcher` trait for test injection | Can be kept for tests or replaced with litchee's `base_url` override for mock server |
| `account.rs` | 0 | Empty file | Delete |

### Data models in `kindle_chess/src/models/` that need mapping:

| Current type | litchee type | Notes |
|-------------|-------------|-------|
| `TokenInfo` | `LichessToken` | litchee uses `Secret<String>` for access_token; needs conversion for JSON persistence |
| `LichessUser` | `LichessUserExtended` (wraps `LichessUser`) | litchee's `LichessUser` has more fields; the app only uses `id` and `username` |
| `Puzzle` | `LichessPuzzle` | litchee's fields are mostly `Option<>`; app's are not. Need unwrapping/defaulting |
| `PuzzleResponse` | `LichessPuzzleAndGame` | Same shape: `{ game, puzzle }` |
| `GameData` / `GameDataList` | `LichessNowPlayingGame` / `LichessNowPlaying` | litchee uses `LichessColor` enum, `Option<>` fields |
| `GameFullEvent`, `GameStateEvent`, `GameOverEvent`, etc. | `LichessGameFull`, `LichessGameState`, `LichessBoardEvent` | litchee wraps these in a single `LichessBoardEvent` enum |
| `PlayedBy` / `PlayedByPlayer` / `PlayedByAi` | `LichessGameEventPlayer` | litchee uses a single struct with `Option<>` fields for both human and AI |
| `StreamEvent` (event stream) | `LichessIncomingEvent` | Similar enum, different variant names |

---

## 2. litchee Feature Coverage vs. App Needs

| App feature | litchee method | Covered? |
|------------|---------------|----------|
| OAuth2 PKCE auth URL | `client.oauth().authorization_url(&AuthorizationRequest{...})` | Yes |
| OAuth2 code exchange | `client.oauth().exchange_code(&CodeExchange{...})` | Yes |
| OAuth2 token revoke | `client.oauth().revoke_token()` | Yes |
| Account profile | `client.account().profile()` | Yes |
| Daily puzzle | `client.puzzles().daily()` | Yes |
| Puzzle by ID | `client.puzzles().get(id)` | Yes |
| Next puzzle (with filters) | `client.puzzles().next(angle, difficulty, color)` | Yes |
| Puzzle activity stream | `client.puzzles().activity(max, before, since)` | Yes |
| Ongoing games list | `client.games().now_playing(nb)` | Yes |
| Board game stream | `client.board().stream_game(game_id)` | Yes |
| Board game move | `client.board().make_move(game_id, move, offering_draw)` | Yes |
| Board game resign | `client.board().resign(game_id)` | Yes |
| Board game abort | `client.board().abort(game_id)` | Yes |
| Event stream (game start/finish) | `client.board().stream_events()` | Yes |
| Scopes: `challenge:read`, `challenge:write`, `bot:play`, `board:play`, `puzzle:read` | `Scope::ChallengeRead`, `Scope::ChallengeWrite`, `Scope::BotPlay`, `Scope::BoardPlay`, `Scope::PuzzleRead` | Yes, all present |
| Custom HTTP client injection | `LichessClientBuilder::http_client(reqwest::Client)` | Yes |
| Base URL override (for testing) | `LichessClientBuilder::base_url(&Url)` | Yes |
| Retry policy (429 handling) | `LichessClientBuilder::retry_policy(RetryPolicy)` | Yes (bonus) |
| NDJSON stream max line size | `LichessClientBuilder::max_line_bytes(usize)` | Yes (bonus) |

**All endpoints the app uses are covered.**

---

## 3. Target Compatibility Assessment (armv7-unknown-linux-musleabi)

### 3.1 Rust version

- Project uses: `rustc 1.99.0`, edition 2024
- litchee requires: `rust-version = "1.99"`, edition 2024
- **Compatible.**

### 3.2 TLS backend (CRITICAL RISK)

- Current: `reqwest 0.12.28` with `rustls-tls` feature (uses `ring` crypto backend)
- litchee requires: `reqwest 0.13.4` with `rustls` feature (uses `aws-lc-rs` crypto backend)

**reqwest 0.13 changes:**
- Feature renamed: `rustls-tls` -> `rustls`
- Crypto provider changed: `ring` -> `aws-lc-rs` (which depends on `aws-lc-sys`, a C/C++ library)
- `aws-lc-sys` requires `cmake`, `cc`, and a C compiler for cross-compilation
- On `armv7-unknown-linux-musleabi`, this should work with `cross` (which provides a Docker image with the right toolchain), but it adds build complexity and time

**Mitigation options:**
1. **Spike test first**: Try `cross build --target armv7-unknown-linux-musleabi` with litchee added. If `aws-lc-sys` builds, we're good.
2. **If aws-lc-sys fails**: litchee's `LichessClientBuilder::http_client()` accepts a pre-configured `reqwest::Client`. We could potentially use `reqwest 0.12` with `ring` for our own client and pass it in. But this is fragile — litchee's internal types reference `reqwest 0.13` types.
3. **Alternative**: Use `reqwest 0.13` with the `rustls` feature but configure `aws-lc-rs` with the `bindgen` feature disabled (pre-generated bindings). This may work for `armv7`.

### 3.3 Binary size

- Current dependency count: 342 crates in Cargo.lock
- litchee adds: ~15 direct deps (async-stream, base64, bytes, futures-core, futures-util, percent-encoding, rand, reqwest, serde, serde_json, serde_urlencoded, sha2, thiserror, tokio, url)
- Most of these are already in the dependency tree (reqwest, serde, tokio, etc.)
- New transitive deps from litchee: `async-stream`, `serde_urlencoded`, `thiserror 2.x`, `rand 0.10`
- reqwest 0.13 upgrade may add: `aws-lc-sys`, `aws-lc-rs` (large C/C++ build)
- **Estimated binary size increase**: +2-5 MB (mostly from aws-lc-rs static linking)
- Kindle Paperwhite 4 has ~4 GB storage; this is acceptable.

### 3.4 Runtime memory

- litchee is a thin wrapper; it doesn't add significant runtime memory overhead
- The `LichessClient` is cheap to clone (shares HTTP client internally)
- Streaming uses `BoxStream` which is zero-cost

---

## 4. Implementation Plan

### Phase 0: Spike (1-2 hours)

**Goal**: Prove litchee compiles and links for `armv7-unknown-linux-musleabi`.

1. Add `litchee = "0.2"` to `Cargo.toml` (temporarily, alongside existing deps)
2. Run: `cd kindle_chess && RUSTFLAGS="-C target-feature=+crt-static" cross build --target armv7-unknown-linux-musleabi --release`
3. If it builds: proceed to Phase 1
4. If `aws-lc-sys` fails: try adding `aws-lc-rs` with `bindgen` feature disabled, or investigate `cross` image customization
5. Also build for native target and check binary size delta: `cargo build --release && ls -lh target/release/kindle-hello`

### Phase 1: Adapter Layer (new module: `src/api/litchee_adapter.rs`)

**Goal**: Create a thin adapter that wraps `LichessClient` and exposes the same interface the app currently uses, so call sites don't change immediately.

```rust
// New file: src/api/litchee_adapter.rs
// Wraps litchee::LichessClient and maps DTOs to app types

pub struct LitcheeAdapter {
    client: litchee::LichessClient,
}

impl LitcheeAdapter {
    pub fn new(token: &str) -> Result<Self, litchee::LichessError>;
    pub fn anonymous() -> Self;

    // Puzzle methods — return app's Puzzle type
    pub async fn get_daily_puzzle(&self) -> Result<Puzzle, ApiError>;
    pub async fn get_puzzle_by_id(&self, id: &str) -> Result<Puzzle, ApiError>;
    pub async fn get_next_puzzle(&self, params: &PuzzleParams) -> Result<Puzzle, ApiError>;
    pub async fn get_home_puzzle(&self, params: &PuzzleParams) -> Result<Puzzle, ApiError>;

    // Account
    pub async fn get_user_info(&self) -> Result<LichessUser, ApiError>;

    // Board games
    pub async fn get_ongoing_games(&self, n: u8) -> Result<GameDataList, ApiError>;
    pub async fn stream_game(&self, game_id: &str) -> Result<impl Stream<Item = Result<GameStateStreamEvent, ApiError>>, ApiError>;
    pub async fn move_piece(&self, game_id: &str, uci: &str) -> Result<(), ApiError>;
    pub async fn resign_game(&self, game_id: &str) -> Result<(), ApiError>;
    pub async fn abort_game(&self, game_id: &str) -> Result<(), ApiError>;
}
```

### Phase 2: Replace API internals (modify `src/api/` files)

**Order of replacement** (bottom-up, test after each):

1. **`api.rs` (puzzles)** -> use `LitcheeAdapter` methods
   - Update `get_daily_puzzle`, `get_puzzle_by_id`, `get_next_puzzle`, `get_home_puzzle`
   - Keep `PuzzleFetcher` trait for tests; adapter wraps litchee calls
   - Run existing tests in `tests/api_puzzle.rs`

2. **`oauth.rs` (auth)** -> use litchee's OAuth primitives
   - Replace `OAuth2Client::start_auth_flow()` with `client.oauth().authorization_url()`
   - Replace `OAuth2Client::exchange_code()` with `client.oauth().exchange_code()`
   - Keep axum callback server as-is
   - Keep token persistence (`load_token`, file I/O) as-is
   - Map `LichessToken` <-> `TokenInfo` for storage compatibility
   - Replace `get_user_info()` with `client.account().profile()`

3. **`board.rs` (game play)** -> use litchee's board API
   - Replace `get_ongoing_games()` with `client.games().now_playing(n)`
   - Replace `move_piece()` with `client.board().make_move()`
   - Replace `resign_game()` with `client.board().resign()`
   - Replace `abort_game()` with `client.board().abort()`
   - Replace `stream_game_event()` with `client.board().stream_game()`
   - Map `LichessBoardEvent` -> `GameStateStreamEvent`
   - Map `LichessNowPlayingGame` -> `GameData`
   - Map `LichessGameEventPlayer` -> `PlayedBy`

4. **Delete `traits.rs`** or keep as thin wrapper for test injection

5. **Delete `account.rs`** (empty file)

### Phase 3: Update call sites (screens, app logic)

Files that import from `crate::api::`:

| File | Changes needed |
|------|---------------|
| `src/ui/screens.rs` | Update imports from `crate::api::api`, `crate::api::oauth`, `crate::api::github`, `crate::api::update` |
| `src/app/chess.rs` | Update `BoardAPI` to hold `LitcheeAdapter` instead of raw `TokenInfo` + `LichessUser` |
| `src/models/board_api.rs` | May need minor adjustments if DTO shapes change |
| `src/models/oauth.rs` | Keep `TokenInfo` for storage; add conversion to/from `LichessToken` |
| `src/models/ui.rs` | No direct API imports, but uses types from `models::board_api` |

### Phase 4: Clean up dependencies

Remove from `Cargo.toml` (now provided by litchee):
- `reqwest` (direct dep — litchee re-exports it internally, but we may still need it for GitHub/update client)
- `reqwest-streams` (litchee handles NDJSON streaming internally)
- `oauth2` (litchee handles OAuth internally)
- `sha2` (litchee uses it for PKCE; app uses it for update verification — **keep**)
- `futures` (may still need `futures-util` for `StreamExt`)

Keep:
- `axum` (callback server)
- `tokio` (runtime)
- `serde`, `serde_json` (DTO serialization, token persistence)
- `qrcode` (QR code generation)
- `sha2` (update verification)
- `image` (QR rendering)
- `log`, `env_logger`, `log4rs`

### Phase 5: Testing

1. **Unit tests**: Update `tests/api_puzzle.rs` to test through the adapter
2. **Integration test**: Add a test that verifies `LitcheeAdapter` can be constructed with a mock base URL
3. **Manual test on dev machine**: `./run-dev.sh` — verify auth flow, puzzle fetch, game play
4. **Cross-compile**: `cross build --target armv7-unknown-linux-musleabi --release`
5. **Deploy to Kindle**: `./flash.sh` — verify on-device

---

## 5. Dependency Changes

### Cargo.toml diff (planned)

```diff
 [dependencies]
 axum = "0.8.4"
+# NEW: litchee replaces hand-rolled lichess API client
+litchee = "0.2"
+# NEW: needed to consume litchee's streams
+futures-util = "0.3"
 exitfailure = "0.5.1"
 env_logger = { version = "0.11", default-features = false }
 fontdue = "0.9"
 image = "0.25.8"
-futures = "0.3.31"
 local-ip-address = "0.6.5"
 log = "0.4.27"
 log4rs = "1.3.0"
-oauth2 = "5.0.0"
 qrcode = "0.14.1"
+# reqwest stays for GitHub update client; version bumped to 0.13 for consistency
+-reqwest = { version = "0.12.23", default-features = false, features = ["json", "rustls-tls", "stream"]}
 reqwest = { version = "0.13", default-features = false, features = ["json", "rustls", "stream"]}
-reqwest-streams = { version = "0.12.0", features=["json"] }
 semver = "1.0"
 sha2 = "0.10"
 serde = { version = "1.0", features = ["derive"] }
 serde_json = "1.0.145"
 tokio = { version = "1", features = ["full"] }
 tokio-util = "0.7.16"
 tower-http = { version = "0.5", features = ["cors"] }
 url = "2.5.7"
 uuid = { version = "1.18.1", features = ["v4"]}
 x11rb = { version = "0.13", default-features = false }
 dotenv = "0.15"
```

### Removed:
- `oauth2 = "5.0.0"` — replaced by litchee's built-in OAuth
- `reqwest-streams` — replaced by litchee's NDJSON streaming
- `futures = "0.3.31"` — replaced by `futures-util` (lighter, only what's needed)

### Version bumps:
- `reqwest`: 0.12 -> 0.13 (required by litchee; `rustls-tls` -> `rustls` feature rename)

---

## 6. Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| `aws-lc-sys` fails to cross-compile for armv7-musl | **HIGH** | Spike test first. If it fails, consider: (a) custom `cross` image with cmake, (b) use `LichessClientBuilder::http_client()` with a reqwest 0.12 client (fragile), (c) contribute a patch to litchee to make reqwest version configurable |
| Binary size increase | MEDIUM | Acceptable on Kindle (4GB storage). Monitor with `ls -lh` |
| DTO shape mismatches cause runtime parse errors | MEDIUM | litchee is synced to the official OpenAPI spec (v2.0.176). The app's parse bug was likely due to spec drift — litchee tracks it automatically |
| OAuth flow behavior changes | LOW | litchee's OAuth is standard PKCE; the callback server stays ours. Token format is standard OAuth2 |
| Breaking changes in litchee (it's 0.x) | LOW | Pin exact version (`=0.2.1`). litchee follows semver-ish; 0.x means breaking changes possible in minor bumps |
| `Secret<String>` in `LichessToken` complicates persistence | LOW | Map to/from `TokenInfo` (plain String) at the adapter boundary |

---

## 7. Rollback Plan

If the migration fails (e.g., cross-compilation blocked):

1. **Revert Cargo.toml** to remove litchee
2. **Revert `src/api/` files** from git
3. **Fix the original parse bug** instead — the root cause is likely a Lichess API response shape change. Check the Lichess API changelog (v2.0.176) for breaking changes to `/api/puzzle/daily`, `/api/puzzle/{id}`, `/api/account`, or `/api/board/game/stream/{id}` response schemas
4. The hand-rolled API is ~1000 lines; fixing the parse bug is likely a <50 line change

---

## 8. Estimated Effort

| Phase | Effort | Notes |
|-------|--------|-------|
| Phase 0: Spike | 1-2 hours | Cross-compile test. Go/no-go gate |
| Phase 1: Adapter | 2-3 hours | New module, DTO mapping functions |
| Phase 2: Replace internals | 3-4 hours | File-by-file replacement, test after each |
| Phase 3: Update call sites | 1-2 hours | Import changes, minor type adjustments |
| Phase 4: Clean up deps | 30 min | Cargo.toml cleanup, `cargo build` verification |
| Phase 5: Testing | 2-3 hours | Unit tests, integration, dev machine, cross-compile, Kindle deploy |
| **Total** | **10-15 hours** | Plus Kindle deploy/test cycle |

---

## 9. Recommendation

**Proceed with the spike (Phase 0) immediately.** If `aws-lc-sys` cross-compiles for `armv7-unknown-linux-musleabi`, the migration is viable and recommended — it eliminates ~1000 lines of hand-rolled API code, fixes the parse bug permanently (litchee tracks the OpenAPI spec), and gives the app access to all 184 Lichess endpoints for free.

If the spike fails on `aws-lc-sys`, fall back to fixing the existing parse bug directly (likely a small change to the serde DTOs in `models/board_api.rs` and `models/puzzle.rs`).
