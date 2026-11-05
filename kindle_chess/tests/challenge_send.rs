//! Integration tests for `LitcheeAdapter::send_challenge` (POST /api/challenge/{user})
//! against a wiremock mock of lichess.org. Verifies each ChallengeMode maps to the
//! correct form fields (no clock / clock.limit+clock.increment / days) and that
//! casual + random color are always sent.

use kindli_chess::api::litchee_adapter::LitcheeAdapter;
use kindli_chess::models::board_api::ChallengeMode;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn adapter(server: &MockServer) -> LitcheeAdapter {
    let base = server.uri().parse().expect("mock uri is a valid url");
    LitcheeAdapter::with_base_url(Some("test-token"), &base).expect("adapter builds")
}

fn challenge_body(id: &str) -> String {
    format!(
        r#"{{"id":"{id}","url":"https://lichess.org/{id}","status":"created",
            "challenger":{{"id":"me","name":"Me"}},"dest_user":{{"id":"alice","name":"Alice"}},
            "rated":false,"speed":"blitz"}}"#
    )
}

/// Mount POST /api/challenge/{username} requiring rated=false + color=random
/// plus the given extra body fragment, responding with challenge id `ch123`.
async fn mount_challenge(server: &MockServer, username: &str, extra_body_fragment: &str) {
    Mock::given(method("POST"))
        .and(path(format!("/api/challenge/{username}")))
        .and(body_string_contains("rated=false"))
        .and(body_string_contains("color=random"))
        .and(body_string_contains(extra_body_fragment))
        .respond_with(ResponseTemplate::new(200).set_body_string(challenge_body("ch123")))
        .mount(server)
        .await;
}

#[tokio::test]
async fn realtime_mode_sends_clock_600_plus_5() {
    let server = MockServer::start().await;
    // wiremock matches all fragments against the same body; mount twice is
    // not needed — use a single fragment check per concern by mounting the
    // strictest one and inspecting the recorded body for the other.
    Mock::given(method("POST"))
        .and(path("/api/challenge/Alice"))
        .and(body_string_contains("rated=false"))
        .and(body_string_contains("color=random"))
        .and(body_string_contains("clock.limit=600"))
        .respond_with(ResponseTemplate::new(200).set_body_string(challenge_body("ch123")))
        .mount(&server)
        .await;

    let ch = adapter(&server)
        .send_challenge("Alice", &ChallengeMode::RealTime)
        .await
        .unwrap();

    assert_eq!(ch.id, "ch123");
    let requests = server.received_requests().await.expect("request recorded");
    let body = String::from_utf8_lossy(&requests[0].body);
    assert!(body.contains("clock.increment=5"), "body: {body}");
}

#[tokio::test]
async fn correspondence_mode_sends_days() {
    let server = MockServer::start().await;
    mount_challenge(&server, "Alice", "days=7").await;

    adapter(&server)
        .send_challenge("Alice", &ChallengeMode::Correspondence(7))
        .await
        .unwrap();
}

#[tokio::test]
async fn unlimited_mode_sends_no_clock_and_no_days() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/challenge/Alice"))
        .and(body_string_contains("rated=false"))
        .respond_with(ResponseTemplate::new(200).set_body_string(challenge_body("ch9")))
        .mount(&server)
        .await;

    let ch = adapter(&server)
        .send_challenge("Alice", &ChallengeMode::Unlimited)
        .await
        .unwrap();

    assert_eq!(ch.id, "ch9");
    // Inspect the actual request body: unlimited must carry neither clock
    // nor days fields.
    let requests = server.received_requests().await.expect("request recorded");
    let body = String::from_utf8_lossy(&requests[0].body);
    assert!(!body.contains("clock"), "unexpected clock in: {body}");
    assert!(!body.contains("days"), "unexpected days in: {body}");
}

#[tokio::test]
async fn send_challenge_requires_authentication() {
    let server = MockServer::start().await;
    let base = server.uri().parse().unwrap();
    let anon = LitcheeAdapter::with_base_url(None, &base).unwrap();

    let err = anon
        .send_challenge("Alice", &ChallengeMode::RealTime)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("authentication"));
}
