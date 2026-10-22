//! HTTP seam for dependency injection in tests.
//!
//! API functions take a `&impl PuzzleFetcher` instead of calling `reqwest`
//! directly. The real `ReqwestFetcher` performs the request and hands back the
//! response body as a `String`; tests substitute a mock that returns canned
//! bodies, so the JSON-parsing and selection logic can be exercised offline.
//!
//! The seam is the response *body*, not `reqwest::Response`: a `Response` can
//! not be synthesised without a live connection, so a trait over it is not
//! mockable. A `String` body is.

use crate::models::oauth::TokenInfo;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Fetches HTTP response bodies. One method per request shape used by the
/// puzzle endpoints: anonymous GET and bearer-authenticated GET.
pub trait PuzzleFetcher: Send + Sync {
    /// GET `url` with no auth; return the response body text.
    fn get(&self, url: &str) -> impl std::future::Future<Output = Result<String, BoxError>> + Send;

    /// GET `url` with a bearer token; return the response body text.
    /// Errors (including non-2xx) if `token` is present and the status is not
    /// successful, so callers can fall back to anonymous requests.
    fn get_auth(
        &self,
        url: &str,
        token: &TokenInfo,
    ) -> impl std::future::Future<Output = Result<String, BoxError>> + Send;
}

/// Production `PuzzleFetcher` backed by `reqwest`.
#[derive(Default)]
pub struct ReqwestFetcher;

impl ReqwestFetcher {
    pub fn new() -> Self {
        Self
    }
}

impl PuzzleFetcher for ReqwestFetcher {
    async fn get(&self, url: &str) -> Result<String, BoxError> {
        Ok(reqwest::get(url).await?.text().await?)
    }

    async fn get_auth(&self, url: &str, token: &TokenInfo) -> Result<String, BoxError> {
        let response = reqwest::Client::new()
            .get(url)
            .bearer_auth(&token.access_token)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(format!("request failed: {}", response.status()).into());
        }
        Ok(response.text().await?)
    }
}
