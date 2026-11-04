//! Integration tests: puzzle API parsing driven by the mock fetcher.
//!
//! Two layers are covered:
//! - the `PuzzleFetcher` seam (`get_daily_puzzle_with` / `get_puzzle_by_id_with`)
//!   proves the offline parse paths against canned bodies, and
//! - the `LitcheeAdapter` (the production path) is exercised end-to-end
//!   against a wiremock server standing in for `lichess.org`, proving the
//!   litchee client wiring, DTO decoding, and mapping to app types.

mod helpers;

use helpers::mock_http::MockFetcher;
use kindli_chess::api::api::{get_daily_puzzle_with, get_puzzle_by_id_with};
use kindli_chess::api::litchee_adapter::LitcheeAdapter;
use kindli_chess::models::puzzle::PuzzleParams;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A complete `/puzzle/daily` response body, including the required `game`
/// wrapper that `PuzzleResponse` deserializes.
fn daily_body() -> &'static str {
    r#"{
        "game": {
            "id": "g123",
            "perf": { "key": "blitz", "name": "Blitz" },
            "rated": true,
            "players": [],
            "pgn": "e4 e5",
            "clock": "5+0"
        },
        "puzzle": {
            "id": "test123",
            "rating": 1500,
            "plays": 100,
            "solution": ["e2e4", "e7e5"],
            "themes": ["tactics"],
            "initialPly": 0,
            "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "lastMove": "d2d4"
        }
    }"#
}

fn daily_url() -> String {
    format!("{}/puzzle/daily", env!("LICHESS_API_BASE"))
}

#[tokio::test]
async fn daily_puzzle_parses_from_mock_body() {
    let url = daily_url();
    let mock = MockFetcher::new().with_body(&url, daily_body());

    let puzzle = get_daily_puzzle_with(&mock).await.unwrap();

    assert_eq!(puzzle.id, "test123");
    assert_eq!(puzzle.rating, 1500);
    assert_eq!(puzzle.solution, vec!["e2e4", "e7e5"]);
    assert_eq!(
        puzzle.fen.as_deref(),
        Some("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
    );
    assert_eq!(puzzle.last_move.as_deref(), Some("d2d4"));
    assert_eq!(mock.request_count(&url), 1);
}

#[tokio::test]
async fn daily_puzzle_errors_on_fetch_error() {
    let url = daily_url();
    let mock = MockFetcher::new().with_error(&url, "network down");

    assert!(get_daily_puzzle_with(&mock).await.is_err());
}

#[tokio::test]
async fn daily_puzzle_errors_on_malformed_json() {
    let url = daily_url();
    let mock = MockFetcher::new().with_body(&url, "{ not json");

    assert!(get_daily_puzzle_with(&mock).await.is_err());
}

#[tokio::test]
async fn puzzle_by_id_hits_the_id_url_and_parses() {
    let id = "abc99";
    let url = format!("{}/puzzle/{}", env!("LICHESS_API_BASE"), id);
    let mock = MockFetcher::new().with_body(&url, daily_body());

    let puzzle = get_puzzle_by_id_with(&mock, id).await.unwrap();

    assert_eq!(puzzle.id, "test123");
    assert_eq!(mock.request_count(&url), 1);
}

// ~~~~~~~~~~~~~~~~ ADAPTER TESTS (wiremock, production path) ~~~~~~~~~~~~~~~~

/// A complete `LichessPuzzleAndGame` body in the live API's camelCase wire
/// format — what `/api/puzzle/daily` and `/api/puzzle/{id}` return.
fn adapter_body() -> &'static str {
    r#"{"game":{"id":"g123","players":[]},
        "puzzle":{"id":"test123","rating":1500,"plays":100,
            "solution":["e2e4","e7e5"],"themes":["tactics"],"initialPly":4,
            "fen":"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "lastMove":"d2d4"}}"#
}

/// The `/api/puzzle/next` body: a puzzle id with no fen/lastMove, forcing the
/// adapter's re-fetch-by-id step.
fn next_body() -> &'static str {
    r#"{"game":{"id":"g999","players":[]},
        "puzzle":{"id":"next456","rating":1600,"plays":10,
            "solution":["f2f3"],"themes":["endgame"],"initialPly":8}}"#
}

/// The full record for the puzzle `/api/puzzle/next` handed out, served by the
/// `/api/puzzle/next456` re-fetch.
fn next_full_body() -> &'static str {
    r#"{"game":{"id":"g999","players":[]},
        "puzzle":{"id":"next456","rating":1600,"plays":10,
            "solution":["f2f3"],"themes":["endgame"],"initialPly":8,
            "fen":"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "lastMove":"e7e5"}}"#
}

fn adapter(server: &MockServer) -> LitcheeAdapter {
    let base = server.uri().parse().expect("mock uri is a valid url");
    LitcheeAdapter::with_base_url(None, &base).expect("adapter builds against mock base url")
}

async fn mount_ok(server: &MockServer, endpoint: &str, body: &str) {
    Mock::given(method("GET"))
        .and(path(endpoint))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
}

#[tokio::test]
async fn adapter_daily_puzzle_parses_and_maps_from_mock_server() {
    let server = MockServer::start().await;
    mount_ok(&server, "/api/puzzle/daily", adapter_body()).await;

    let puzzle = adapter(&server).get_daily_puzzle().await.unwrap();

    assert_eq!(puzzle.id, "test123");
    assert_eq!(puzzle.rating, 1500);
    assert_eq!(puzzle.plays, 100);
    assert_eq!(puzzle.initial_ply, 4);
    assert_eq!(puzzle.solution, vec!["e2e4", "e7e5"]);
    assert_eq!(puzzle.themes, vec!["tactics"]);
    assert_eq!(
        puzzle.fen.as_deref(),
        Some("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
    );
    assert_eq!(puzzle.last_move.as_deref(), Some("d2d4"));
}

#[tokio::test]
async fn adapter_puzzle_by_id_hits_the_id_path() {
    let server = MockServer::start().await;
    mount_ok(&server, "/api/puzzle/abc99", adapter_body()).await;

    let puzzle = adapter(&server).get_puzzle_by_id("abc99").await.unwrap();

    assert_eq!(puzzle.id, "test123");
    assert_eq!(puzzle.rating, 1500);
}

#[tokio::test]
async fn adapter_next_puzzle_refetches_the_full_record() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/puzzle/next"))
        .and(query_param("difficulty", "normal"))
        .respond_with(ResponseTemplate::new(200).set_body_string(next_body()))
        .mount(&server)
        .await;
    mount_ok(&server, "/api/puzzle/next456", next_full_body()).await;

    let puzzle = adapter(&server)
        .get_next_puzzle(&PuzzleParams::default())
        .await
        .unwrap();

    // The id comes from /next, the position from the /puzzle/{id} re-fetch.
    assert_eq!(puzzle.id, "next456");
    assert_eq!(
        puzzle.fen.as_deref(),
        Some("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
    );
    assert_eq!(puzzle.last_move.as_deref(), Some("e7e5"));
}

#[tokio::test]
async fn adapter_maps_missing_optionals_to_defaults() {
    let body = r#"{"game":{"id":"g1","players":[]},
        "puzzle":{"id":"bare","solution":[],"themes":[]}}"#;
    let server = MockServer::start().await;
    mount_ok(&server, "/api/puzzle/daily", body).await;

    let puzzle = adapter(&server).get_daily_puzzle().await.unwrap();

    assert_eq!(puzzle.rating, 0);
    assert_eq!(puzzle.plays, 0);
    assert_eq!(puzzle.initial_ply, 0);
    assert_eq!(puzzle.fen, None);
    assert_eq!(puzzle.last_move, None);
}

#[tokio::test]
async fn adapter_daily_puzzle_errors_on_server_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/puzzle/daily"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    assert!(adapter(&server).get_daily_puzzle().await.is_err());
}

#[tokio::test]
async fn adapter_constructed_with_mock_base_url_sends_bearer_token() {
    use wiremock::matchers::header;

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/puzzle/daily"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string(adapter_body()))
        .mount(&server)
        .await;

    let base = server.uri().parse().expect("mock uri is a valid url");
    let auth_adapter =
        LitcheeAdapter::with_base_url(Some("test-token"), &base).expect("adapter builds");
    let puzzle = auth_adapter.get_daily_puzzle().await.unwrap();

    // The header matcher above only matches with the bearer token present;
    // a successful decode proves the authenticated request actually hit it.
    assert_eq!(puzzle.id, "test123");
}

#[tokio::test]
async fn adapter_home_puzzle_returns_daily_when_daily_not_played() {
    let activity = concat!(
        r#"{"date":1,"win":true,"puzzle":{"id":"other999","solution":[],"themes":[]}}"#,
        "\n"
    );
    let server = MockServer::start().await;
    mount_ok(&server, "/api/puzzle/daily", adapter_body()).await;
    mount_ok(&server, "/api/puzzle/activity", activity).await;

    let puzzle = adapter(&server)
        .get_home_puzzle(&PuzzleParams::default())
        .await
        .unwrap();

    assert_eq!(puzzle.id, "test123");
}

#[tokio::test]
async fn adapter_home_puzzle_returns_next_when_daily_already_played() {
    // Activity lists today's daily ("test123"), so the home button must hand
    // out a fresh /next puzzle instead.
    let activity = concat!(
        r#"{"date":1,"win":true,"puzzle":{"id":"test123","solution":[],"themes":[]}}"#,
        "\n"
    );
    let server = MockServer::start().await;
    mount_ok(&server, "/api/puzzle/daily", adapter_body()).await;
    mount_ok(&server, "/api/puzzle/activity", activity).await;
    Mock::given(method("GET"))
        .and(path("/api/puzzle/next"))
        .respond_with(ResponseTemplate::new(200).set_body_string(next_body()))
        .mount(&server)
        .await;
    mount_ok(&server, "/api/puzzle/next456", next_full_body()).await;

    let puzzle = adapter(&server)
        .get_home_puzzle(&PuzzleParams::default())
        .await
        .unwrap();

    // The fresh /next puzzle (id from /next, position from its re-fetch).
    assert_eq!(puzzle.id, "next456");
    assert!(puzzle.fen.is_some());
    let requests = server.received_requests().await.unwrap_or_default();
    assert!(
        requests.iter().any(|r| r.url.path() == "/api/puzzle/next"),
        "expected a /api/puzzle/next request among {} received",
        requests.len()
    );
}

#[tokio::test]
async fn adapter_post_puzzle_solution_posts_to_batch_angle() {
    use wiremock::matchers::{body_json, header, query_param};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/puzzle/batch/mix"))
        .and(query_param("nb", "0"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({
            "solutions": [{"id": "test123", "win": true, "rated": true}]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"rounds":[{"id":"test123","win":true,"ratingDiff":12}]}"#,
        ))
        .mount(&server)
        .await;

    let base = server.uri().parse().expect("mock uri is a valid url");
    let adapter =
        LitcheeAdapter::with_base_url(Some("test-token"), &base).expect("adapter builds");

    let rounds = adapter
        .post_puzzle_solution("mix", "test123", true, 0)
        .await
        .unwrap();

    assert_eq!(rounds.len(), 1);
    assert_eq!(rounds[0].id, "test123");
    assert_eq!(rounds[0].win, Some(true));
    assert_eq!(rounds[0].rating_diff, Some(12));
}

#[tokio::test]
async fn adapter_post_puzzle_solution_errors_on_403() {
    // Tokens issued before the puzzle:write scope get a 403 — the caller
    // degrades to local-only play, so the error must surface as Err.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/puzzle/batch/mix"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let result = adapter(&server)
        .post_puzzle_solution("mix", "test123", true, 0)
        .await;

    assert!(result.is_err());
}
