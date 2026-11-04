//! Mock `PuzzleFetcher` for testing API parse logic without a network.
//!
//! Stores canned response bodies keyed by URL and returns them from the
//! `PuzzleFetcher` trait, so API functions can be driven offline.

use std::collections::HashMap;
use std::sync::Mutex;

use kindli_chess::api::traits::PuzzleFetcher;
use kindli_chess::models::oauth::TokenInfo;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A canned fetcher: maps URL -> body string (Ok) or error message (Err).
#[derive(Default)]
pub struct MockFetcher {
    bodies: HashMap<String, Result<String, String>>,
    counts: Mutex<HashMap<String, usize>>,
}

impl MockFetcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a raw body string for `url` (e.g. NDJSON or malformed JSON).
    pub fn with_body(mut self, url: &str, body: &str) -> Self {
        self.bodies.insert(url.to_string(), Ok(body.to_string()));
        self
    }

    /// Register an error response for `url`.
    pub fn with_error(mut self, url: &str, msg: &str) -> Self {
        self.bodies.insert(url.to_string(), Err(msg.to_string()));
        self
    }

    /// How many times `url` was fetched.
    pub fn request_count(&self, url: &str) -> usize {
        *self.counts.lock().unwrap().get(url).unwrap_or(&0)
    }

    fn resolve(&self, url: &str) -> Result<String, BoxError> {
        *self
            .counts
            .lock()
            .unwrap()
            .entry(url.to_string())
            .or_insert(0) += 1;
        match self.bodies.get(url) {
            Some(Ok(body)) => Ok(body.clone()),
            Some(Err(msg)) => Err(msg.clone().into()),
            None => Err(format!("no mock response for URL: {url}").into()),
        }
    }
}

impl PuzzleFetcher for MockFetcher {
    async fn get(&self, url: &str) -> Result<String, BoxError> {
        self.resolve(url)
    }

    async fn get_auth(&self, url: &str, _token: &TokenInfo) -> Result<String, BoxError> {
        self.resolve(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_returns_configured_body() {
        let mock = MockFetcher::new().with_body("http://x/a", "hello");
        assert_eq!(mock.get("http://x/a").await.unwrap(), "hello");
        assert_eq!(mock.request_count("http://x/a"), 1);
    }

    #[tokio::test]
    async fn get_errors_for_unconfigured_url() {
        let mock = MockFetcher::new();
        assert!(mock.get("http://x/missing").await.is_err());
    }

    #[tokio::test]
    async fn with_error_is_returned_as_err() {
        let mock = MockFetcher::new().with_error("http://x/a", "boom");
        assert!(mock.get("http://x/a").await.is_err());
    }
}
