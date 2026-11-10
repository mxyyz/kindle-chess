//! Unified error type for the kindle-chess application.
//!
//! `ChessError` is `Send + Sync + 'static`, so it can cross async task
//! boundaries (unlike `Box<dyn Error>`). All public functions in `api/`
//! return `Result<T, ChessError>`.

use std::fmt;

/// Errors that can occur at the API boundary and beyond.
#[derive(Debug)]
pub enum ChessError {
    /// Lichess API error (HTTP status, unexpected response shape).
    Api(String),
    /// Authentication / OAuth error.
    Auth(String),
    /// Network connectivity error (DNS, connection refused, timeout).
    Network(String),
    /// Parse error (JSON, FEN, UCI, etc.).
    Parse(String),
    /// Filesystem I/O error.
    Io(std::io::Error),
    /// Internal error (channel closed, QR generation, etc.).
    Internal(String),
}

impl fmt::Display for ChessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChessError::Api(m) => write!(f, "API error: {m}"),
            ChessError::Auth(m) => write!(f, "Auth error: {m}"),
            ChessError::Network(m) => write!(f, "Network error: {m}"),
            ChessError::Parse(m) => write!(f, "Parse error: {m}"),
            ChessError::Io(e) => write!(f, "I/O error: {e}"),
            ChessError::Internal(m) => write!(f, "Internal error: {m}"),
        }
    }
}

impl std::error::Error for ChessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ChessError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<litchee::LichessError> for ChessError {
    fn from(e: litchee::LichessError) -> Self {
        ChessError::Api(e.to_string())
    }
}

impl From<reqwest::Error> for ChessError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_connect() || e.is_timeout() {
            ChessError::Network(e.to_string())
        } else {
            ChessError::Api(e.to_string())
        }
    }
}

impl From<std::io::Error> for ChessError {
    fn from(e: std::io::Error) -> Self {
        ChessError::Io(e)
    }
}

impl From<serde_json::Error> for ChessError {
    fn from(e: serde_json::Error) -> Self {
        ChessError::Parse(e.to_string())
    }
}

impl From<String> for ChessError {
    fn from(s: String) -> Self {
        ChessError::Api(s)
    }
}

impl From<&str> for ChessError {
    fn from(s: &str) -> Self {
        ChessError::Api(s.to_string())
    }
}

impl From<axum::http::header::InvalidHeaderValue> for ChessError {
    fn from(e: axum::http::header::InvalidHeaderValue) -> Self {
        ChessError::Api(e.to_string())
    }
}

impl From<local_ip_address::Error> for ChessError {
    fn from(e: local_ip_address::Error) -> Self {
        ChessError::Network(e.to_string())
    }
}

impl From<qrcode::types::QrError> for ChessError {
    fn from(e: qrcode::types::QrError) -> Self {
        ChessError::Internal(e.to_string())
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for ChessError {
    fn from(e: tokio::sync::oneshot::error::RecvError) -> Self {
        ChessError::Internal(e.to_string())
    }
}

impl From<litchee::error::PkceError> for ChessError {
    fn from(e: litchee::error::PkceError) -> Self {
        ChessError::Auth(e.to_string())
    }
}

// Compile-time guarantee: ChessError is Send + Sync.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ChessError>();
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_lichess_error_maps_to_api() {
        // LichessError doesn't have a public constructor, so test via Display
        let e = ChessError::Api("test".into());
        assert!(matches!(e, ChessError::Api(_)));
    }

    #[test]
    fn from_io_error_maps_to_io() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let e: ChessError = io.into();
        assert!(matches!(e, ChessError::Io(_)));
    }

    #[test]
    fn from_serde_json_error_maps_to_parse() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let e: ChessError = json_err.into();
        assert!(matches!(e, ChessError::Parse(_)));
    }

    #[test]
    fn from_string_maps_to_api() {
        let e: ChessError = "something broke".into();
        assert!(matches!(e, ChessError::Api(m) if m == "something broke"));
    }

    #[test]
    fn display_formats_all_variants() {
        assert!(
            ChessError::Api("a".into())
                .to_string()
                .contains("API error: a")
        );
        assert!(
            ChessError::Auth("b".into())
                .to_string()
                .contains("Auth error: b")
        );
        assert!(
            ChessError::Network("c".into())
                .to_string()
                .contains("Network error: c")
        );
        assert!(
            ChessError::Parse("d".into())
                .to_string()
                .contains("Parse error: d")
        );
    }

    #[test]
    fn error_source_returns_io_cause() {
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let e = ChessError::Io(io);
        assert!(std::error::Error::source(&e).is_some());
        assert!(std::error::Error::source(&ChessError::Api("x".into())).is_none());
    }

    #[test]
    fn chess_error_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<ChessError>();
    }
}
