//! Integration tests for e06s01 adapter extensions against a wiremock mock:
//! - `send_challenge` with `rated=true` sends `rated=true` (POST /api/challenge/{user})
//! - `send_challenge_ai` POSTs /api/challenge/ai with the level and returns the game id
//! - `create_open_challenge` POSTs /api/challenge/open with rated + time control

use kindli_chess::api::litchee_adapter::LitcheeAdapter;
use kindli_chess::models::board_api::ChallengeMode;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn adapter(server: &MockServer) -> LitcheeAdapter {
    let base = server.uri().parse().expect("mock uri is a valid url");
    LitcheeAdapter::with_base_url(Some("test-token"), &base).expect("adapter builds")
}

fn challenge_body(id: &str, rated: bool) -> String {
    format!(
        r#"{{"id":"{id}","url":"https://lichess.org/{id}","status":"created",
            "challenger":{{"id":"me","name":"Me"}},"dest_user":{{"id":"alice","name":"Alice"}},
            "rated":{rated},"speed":"blitz"}}"#
    )
}

#[tokio::test]
async fn rated_friend_challenge_sends_rated_true() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/challenge/Alice"))
        .and(body_string_contains("rated=true"))
        .and(body_string_contains("color=random"))
        .and(body_string_contains("clock.limit=600"))
        .respond_with(ResponseTemplate::new(200).set_body_string(challenge_body("chR", true)))
        .mount(&server)
        .await;

    let ch = adapter(&server)
        .send_challenge("Alice", &ChallengeMode::RealTime, true)
        .await
        .unwrap();

    assert_eq!(ch.id, "chR");
    assert!(ch.rated, "summary must carry the rated flag");
}

#[tokio::test]
async fn casual_friend_challenge_sends_rated_false() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/challenge/Alice"))
        .and(body_string_contains("rated=false"))
        .respond_with(ResponseTemplate::new(200).set_body_string(challenge_body("chC", false)))
        .mount(&server)
        .await;

    let ch = adapter(&server)
        .send_challenge("Alice", &ChallengeMode::Unlimited, false)
        .await
        .unwrap();

    assert!(!ch.rated);
}

#[tokio::test]
async fn challenge_ai_posts_to_ai_endpoint_with_level_and_returns_game_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/challenge/ai"))
        .and(body_string_contains("level=5"))
        .and(body_string_contains("clock.limit=600"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            // Live API shape: `variant` is an object, not a string (BUG-2026-11-11T122900).
            r#"{"id":"aiGame42","rated":false,"speed":"blitz","perf":"blitz",
                "status":"started","source":"ai",
                "variant":{"key":"standard","name":"Standard","short":"Std"}}"#,
        ))
        .mount(&server)
        .await;

    let game = adapter(&server)
        .send_challenge_ai(5, &ChallengeMode::RealTime)
        .await
        .unwrap();

    assert_eq!(game.id, "aiGame42");
}

#[tokio::test]
async fn challenge_ai_correspondence_sends_days() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/challenge/ai"))
        .and(body_string_contains("level=3"))
        .and(body_string_contains("days=7"))
        .and(body_string_contains("color=random"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"id":"aiCorr1","status":"started","source":"ai",
                "variant":{"key":"standard","name":"Standard","short":"Std"}}"#,
        ))
        .mount(&server)
        .await;

    let game = adapter(&server)
        .send_challenge_ai(3, &ChallengeMode::Correspondence(7))
        .await
        .unwrap();

    assert_eq!(game.id, "aiCorr1");
}

#[tokio::test]
async fn challenge_ai_response_without_id_errors_cleanly() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/challenge/ai"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"status":"started","variant":{"key":"standard","name":"Standard","short":"Std"}}"#,
        ))
        .mount(&server)
        .await;

    let err = adapter(&server)
        .send_challenge_ai(5, &ChallengeMode::RealTime)
        .await
        .unwrap_err();

    assert!(
        err.to_string().contains("no game id"),
        "expected a missing-id error, got: {err}"
    );
}

#[tokio::test]
async fn create_open_challenge_posts_to_open_endpoint_with_rated() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/challenge/open"))
        .and(body_string_contains("rated=true"))
        .and(body_string_contains("days=3"))
        .respond_with(ResponseTemplate::new(200).set_body_string(challenge_body("chO", true)))
        .mount(&server)
        .await;

    let ch = adapter(&server)
        .create_open_challenge(&ChallengeMode::Correspondence(3), true)
        .await
        .unwrap();

    assert_eq!(ch.id, "chO");
    assert!(ch.rated);
    assert!(!ch.incoming, "open challenge we created is outgoing");
}

#[tokio::test]
async fn challenge_ai_requires_authentication() {
    let server = MockServer::start().await;
    let base = server.uri().parse().unwrap();
    let anon = LitcheeAdapter::with_base_url(None, &base).unwrap();

    let err = anon
        .send_challenge_ai(3, &ChallengeMode::RealTime)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("authentication"));

    let err = anon
        .create_open_challenge(&ChallengeMode::RealTime, false)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("authentication"));
}
