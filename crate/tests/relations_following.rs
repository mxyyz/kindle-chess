//! Integration tests for `LitcheeAdapter::get_following` (GET /api/rel/following)
//! against a wiremock server standing in for `lichess.org`. The endpoint streams
//! NDJSON — one `LichessUser` object per line — which the adapter collects into
//! a plain list of usernames for the FriendlistScreen.

use kindli_chess::api::litchee_adapter::LitcheeAdapter;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn adapter(server: &MockServer) -> LitcheeAdapter {
    let base = server.uri().parse().expect("mock uri is a valid url");
    LitcheeAdapter::with_base_url(Some("test-token"), &base).expect("adapter builds")
}

/// NDJSON body: two followed users, plus a line missing optional fields only.
fn following_body() -> &'static str {
    "{\"id\":\"alice42\",\"username\":\"Alice42\",\"title\":\"GM\"}\n\
     {\"id\":\"bob7\",\"username\":\"bob7\"}\n"
}

#[tokio::test]
async fn get_following_collects_usernames_from_ndjson_stream() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/rel/following"))
        .respond_with(ResponseTemplate::new(200).set_body_string(following_body()))
        .mount(&server)
        .await;

    let names = adapter(&server).get_following().await.unwrap();

    assert_eq!(names, vec!["Alice42".to_string(), "bob7".to_string()]);
}

#[tokio::test]
async fn get_following_empty_stream_yields_empty_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/rel/following"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(&server)
        .await;

    let names = adapter(&server).get_following().await.unwrap();

    assert!(names.is_empty());
}

#[tokio::test]
async fn get_following_requires_authentication() {
    let server = MockServer::start().await;
    let base = server.uri().parse().unwrap();
    let anon = LitcheeAdapter::with_base_url(None, &base).unwrap();

    let err = anon.get_following().await.unwrap_err();

    assert!(err.to_string().contains("authentication"));
}
