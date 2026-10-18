//! Integration tests: puzzle API parsing driven by the mock fetcher.
//!
//! These exercise the real `get_daily_puzzle_with` / `get_puzzle_by_id_with`
//! parse paths offline, proving the `PuzzleFetcher` seam works end-to-end.

mod helpers;

use helpers::mock_http::MockFetcher;
use kindli_chess::api::api::{get_daily_puzzle_with, get_puzzle_by_id_with};

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
