//! Integration tests for challenge list/accept/decline/cancel against a
//! wiremock mock of lichess.org (GET /api/challenge, POST /api/challenge/{id}/...).

use kindli_chess::api::litchee_adapter::LitcheeAdapter;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn adapter(server: &MockServer) -> LitcheeAdapter {
    let base = server.uri().parse().expect("mock uri is a valid url");
    LitcheeAdapter::with_base_url(Some("test-token"), &base).expect("adapter builds")
}

fn list_body() -> &'static str {
    r#"{
      "in": [
        {"id":"in1","url":"https://lichess.org/in1","status":"created",
         "challenger":{"id":"bob","name":"Bob"},"destUser":{"id":"me","name":"Me"},
         "rated":false,"speed":"rapid"}
      ],
      "out": [
        {"id":"out1","url":"https://lichess.org/","status":"accepted",
         "challenger":{"id":"me","name":"Me"},"destUser":{"id":"alice","name":"Alice"},
         "rated":false,"speed":"blitz"}
      ]
    }"#
}

#[tokio::test]
async fn list_challenges_maps_incoming_and_outgoing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/challenge"))
        .respond_with(ResponseTemplate::new(200).set_body_string(list_body()))
        .mount(&server)
        .await;

    let list = adapter(&server).list_challenges().await.unwrap();

    assert_eq!(list.len(), 2);
    let incoming = list.iter().find(|c| c.incoming).unwrap();
    assert_eq!(incoming.id, "in1");
    assert_eq!(incoming.opponent, "Bob");
    assert_eq!(incoming.status, "created");
    assert_eq!(incoming.game_id(), None);
    let outgoing = list.iter().find(|c| !c.incoming).unwrap();
    assert_eq!(outgoing.opponent, "Alice");
    assert_eq!(outgoing.status, "accepted");
    // url "https://lichess.org/" has an empty last segment -> None
    assert_eq!(outgoing.game_id(), None);
}

#[tokio::test]
async fn accepted_challenge_yields_game_id_from_url() {
    let server = MockServer::start().await;
    let body = r#"{"incoming":[],"out": [
        {"id":"out9","url":"https://lichess.org/abcd1234","status":"accepted",
         "challenger":{"id":"me","name":"Me"},"destUser":{"id":"alice","name":"Alice"},
         "rated":false,"speed":"blitz"}]}"#;
    Mock::given(method("GET"))
        .and(path("/api/challenge"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let list = adapter(&server).list_challenges().await.unwrap();

    assert_eq!(list[0].game_id(), Some("abcd1234"));
}

#[tokio::test]
async fn open_challenge_decodes_as_outgoing_open_and_is_cancellable() {
    // BUG-2026-11-11T123000: an outgoing entry with "destUser": null is an
    // open challenge — it must decode, map to open=true with an empty
    // opponent, and be cancellable like any other outgoing challenge.
    let server = MockServer::start().await;
    let body = r#"{"incoming":[],"out": [
        {"id":"chOpen9","url":"https://lichess.org/","status":"created",
         "challenger":{"id":"me","name":"Me"},"destUser":null,
         "rated":true,"speed":"rapid"}]}"#;
    Mock::given(method("GET"))
        .and(path("/api/challenge"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/challenge/chOpen9/cancel"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;

    let api = adapter(&server);
    let list = api.list_challenges().await.unwrap();
    assert_eq!(list.len(), 1);
    let c = &list[0];
    assert!(!c.incoming, "open challenge we created is outgoing");
    assert!(c.open, "destUser: null must map to open=true");
    assert_eq!(c.opponent, "");
    assert!(c.rated);

    api.cancel_challenge("chOpen9").await.unwrap();
}

#[tokio::test]
async fn accept_decline_cancel_hit_their_endpoints() {
    let server = MockServer::start().await;
    for action in ["accept", "decline", "cancel"] {
        Mock::given(method("POST"))
            .and(path(format!("/api/challenge/ch77/{action}")))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .mount(&server)
            .await;
    }

    let api = adapter(&server);
    api.accept_challenge("ch77").await.unwrap();
    api.decline_challenge("ch77").await.unwrap();
    api.cancel_challenge("ch77").await.unwrap();
}

#[tokio::test]
async fn list_challenges_requires_authentication() {
    let server = MockServer::start().await;
    let base = server.uri().parse().unwrap();
    let anon = LitcheeAdapter::with_base_url(None, &base).unwrap();

    let err = anon.list_challenges().await.unwrap_err();

    assert!(err.to_string().contains("authentication"));
}
