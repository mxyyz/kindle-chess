// Sample implementation for Task t12-chess-error-type
// This file demonstrates the ChessError enum and its implementations

// ============================================================================
// NEW FILE: kindle_chess/src/error.rs
// ============================================================================

use std::fmt;
use std::error::Error;

/// Unified error type for the kindle-chess application.
/// 
/// This enum provides a consistent way to handle errors throughout the application,
/// ensuring that all errors are Send + Sync + 'static, which is required for use
/// across async boundaries.
#[derive(Debug)]
pub enum ChessError {
    /// Error from HTTP API requests
    ApiError(String),
    
    /// Error parsing data (JSON, FEN, UCI, etc.)
    ParseError(String),
    
    /// I/O errors (file operations, etc.)
    IoError(std::io::Error),
    
    /// Network connectivity errors
    NetworkError(String),
    
    /// Authentication-related errors
    AuthError(String),
    
    /// Input validation errors
    ValidationError(String),
    
    /// Internal errors that shouldn't happen in normal operation
    InternalError(String),
    
    /// X11 display errors
    DisplayError(String),
    
    /// Rendering errors
    RenderError(String),
}

impl fmt::Display for ChessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChessError::ApiError(msg) => write!(f, "API Error: {}", msg),
            ChessError::ParseError(msg) => write!(f, "Parse Error: {}", msg),
            ChessError::IoError(err) => write!(f, "I/O Error: {}", err),
            ChessError::NetworkError(msg) => write!(f, "Network Error: {}", msg),
            ChessError::AuthError(msg) => write!(f, "Auth Error: {}", msg),
            ChessError::ValidationError(msg) => write!(f, "Validation Error: {}", msg),
            ChessError::InternalError(msg) => write!(f, "Internal Error: {}", msg),
            ChessError::DisplayError(msg) => write!(f, "Display Error: {}", msg),
            ChessError::RenderError(msg) => write!(f, "Render Error: {}", msg),
        }
    }
}

impl Error for ChessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ChessError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl ChessError {
    /// Create a new ApiError
    pub fn api(msg: impl Into<String>) -> Self {
        ChessError::ApiError(msg.into())
    }
    
    /// Create a new ParseError
    pub fn parse(msg: impl Into<String>) -> Self {
        ChessError::ParseError(msg.into())
    }
    
    /// Create a new NetworkError
    pub fn network(msg: impl Into<String>) -> Self {
        ChessError::NetworkError(msg.into())
    }
    
    /// Create a new AuthError
    pub fn auth(msg: impl Into<String>) -> Self {
        ChessError::AuthError(msg.into())
    }
    
    /// Create a new ValidationError
    pub fn validation(msg: impl Into<String>) -> Self {
        ChessError::ValidationError(msg.into())
    }
    
    /// Create a new InternalError
    pub fn internal(msg: impl Into<String>) -> Self {
        ChessError::InternalError(msg.into())
    }
    
    /// Create a new DisplayError
    pub fn display(msg: impl Into<String>) -> Self {
        ChessError::DisplayError(msg.into())
    }
    
    /// Create a new RenderError
    pub fn render(msg: impl Into<String>) -> Self {
        ChessError::RenderError(msg.into())
    }
    
    /// Check if this is an API error
    pub fn is_api_error(&self) -> bool {
        matches!(self, ChessError::ApiError(_))
    }
    
    /// Check if this is a parse error
    pub fn is_parse_error(&self) -> bool {
        matches!(self, ChessError::ParseError(_))
    }
    
    /// Check if this is a network error
    pub fn is_network_error(&self) -> bool {
        matches!(self, ChessError::NetworkError(_))
    }
    
    /// Check if this is an auth error
    pub fn is_auth_error(&self) -> bool {
        matches!(self, ChessError::AuthError(_))
    }
    
    /// Get the error message as a string slice
    pub fn message(&self) -> &str {
        match self {
            ChessError::ApiError(msg) => msg,
            ChessError::ParseError(msg) => msg,
            ChessError::NetworkError(msg) => msg,
            ChessError::AuthError(msg) => msg,
            ChessError::ValidationError(msg) => msg,
            ChessError::InternalError(msg) => msg,
            ChessError::DisplayError(msg) => msg,
            ChessError::RenderError(msg) => msg,
            ChessError::IoError(_) => "I/O error",
        }
    }
}

// ============================================================================
// From Implementations
// ============================================================================

// From Box<dyn Error> - this is the key implementation that allows
// converting any error into ChessError
impl From<Box<dyn Error + Send + Sync>> for ChessError {
    fn from(err: Box<dyn Error + Send + Sync>) -> Self {
        ChessError::InternalError(err.to_string())
    }
}

// From Box<dyn Error> - for non-Send errors, we still want to be able to convert
impl From<Box<dyn Error>> for ChessError {
    fn from(err: Box<dyn Error>) -> Self {
        ChessError::InternalError(err.to_string())
    }
}

// From std::io::Error
impl From<std::io::Error> for ChessError {
    fn from(err: std::io::Error) -> Self {
        ChessError::IoError(err)
    }
}

// From reqwest::Error
impl From<reqwest::Error> for ChessError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_connect() || err.is_timeout() {
            ChessError::NetworkError(err.to_string())
        } else if err.is_status() {
            ChessError::ApiError(format!("HTTP {}: {}", 
                err.status().map(|s| s.as_u16()).unwrap_or(0),
                err.to_string()))
        } else {
            ChessError::ApiError(err.to_string())
        }
    }
}

// From serde_json::Error
impl From<serde_json::Error> for ChessError {
    fn from(err: serde_json::Error) -> Self {
        ChessError::ParseError(err.to_string())
    }
}

// From url::ParseError
impl From<url::ParseError> for ChessError {
    fn from(err: url::ParseError) -> Self {
        ChessError::ParseError(err.to_string())
    }
}

// From oauth2::Error
impl From<oauth2::Error> for ChessError {
    fn from(err: oauth2::Error) -> Self {
        ChessError::AuthError(err.to_string())
    }
}

// From x11rb::errors::Error
impl From<x11rb::errors::Error> for ChessError {
    fn from(err: x11rb::errors::Error) -> Self {
        ChessError::DisplayError(err.to_string())
    }
}

// From image::ImageError
impl From<image::ImageError> for ChessError {
    fn from(err: image::ImageError) -> Self {
        ChessError::RenderError(err.to_string())
    }
}

// From fontdue::FontError
impl From<fontdue::FontError> for ChessError {
    fn from(err: fontdue::FontError) -> Self {
        ChessError::RenderError(err.to_string())
    }
}

// ============================================================================
// Result Type Alias
// ============================================================================

/// Convenience type alias for functions that return ChessError
pub type ChessResult<T> = Result<T, ChessError>;

// ============================================================================
// SAMPLE: Updated API function to use ChessError
// ============================================================================

// This would be in kindle_chess/src/api/api.rs

use crate::error::ChessResult;
use crate::models::puzzle::Puzzle;

/// Get the daily puzzle, returning ChessError instead of Box<dyn Error>
pub async fn get_daily_puzzle() -> ChessResult<Puzzle> {
    let url = format!("{}/puzzle/daily", env!("LICHESS_API_BASE"));
    let response = reqwest::get(&url).await?;
    let puzzle_response: super::PuzzleResponse = response.json().await?;
    Ok(puzzle_response.puzzle)
}

/// Get a puzzle by ID, returning ChessError
pub async fn get_puzzle_by_id(id: &str) -> ChessResult<Puzzle> {
    let url = format!("{}/puzzle/{}", env!("LICHESS_API_BASE"), id);
    let response = reqwest::get(&url).await?;
    let puzzle_response: super::PuzzleResponse = response.json().await?;
    Ok(puzzle_response.puzzle)
}

// ============================================================================
// SAMPLE: Error handling in screens
// ============================================================================

// This would be in a screen implementation, e.g., kindle_chess/src/ui/screens/puzzle.rs

use crate::error::ChessError;

impl Screen for PuzzleScreen {
    fn handle_event(
        &mut self,
        event: AppEvent,
        display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            AppEvent::PuzzleLoaded(puzzle) => {
                // Handle puzzle loaded
                Ok(Transition::Redraw)
            }
            AppEvent::PuzzleLoadFailed(err) => {
                // Convert string error to ChessError for consistent handling
                let chess_err = ChessError::ApiError(err);
                log::error!("Failed to load puzzle: {}", chess_err);
                // Show error to user (in a real implementation)
                Ok(Transition::Stay)
            }
            // ... other event handlers
        }
    }
}

// ============================================================================
// SAMPLE: Using ChessError in bitboard parsing
// ============================================================================

// This would be in kindle_chess/src/models/bitboard.rs

use crate::error::ChessResult;

impl Bitboards {
    /// Parse FEN, returning ChessError instead of String
    pub fn from_fen(fen: &str) -> ChessResult<Self> {
        if fen.trim() == "startpos" {
            return Ok(Self::starting_position());
        }

        let mut parts = fen.split_whitespace();
        let position = parts.next().ok_or_else(|| {
            ChessError::ParseError("empty FEN".to_string())
        })?;
        
        let side = parts.next().unwrap_or("w");
        let side_to_move = match side {
            "w" => Color::White,
            "b" => Color::Black,
            other => return Err(ChessError::ParseError(
                format!("FEN: bad side-to-move '{}'", other)
            )),
        };
        
        // ... rest of parsing logic, returning ChessError on failures
        
        Ok(bb)
    }
}

// ============================================================================
// SAMPLE: Tests for ChessError
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chess_error_display() {
        let err = ChessError::ApiError("test error".to_string());
        assert_eq!(format!("{}", err), "API Error: test error");
        
        let err = ChessError::ParseError("parse failed".to_string());
        assert_eq!(format!("{}", err), "Parse Error: parse failed");
    }

    #[test]
    fn test_chess_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let chess_err: ChessError = io_err.into();
        assert!(matches!(chess_err, ChessError::IoError(_)));
    }

    #[test]
    fn test_chess_error_from_box_dyn_error() {
        let boxed: Box<dyn Error + Send + Sync> = Box::new(std::io::Error::new(
            std::io::ErrorKind::Other, "test"
        ));
        let chess_err: ChessError = boxed.into();
        assert!(matches!(chess_err, ChessError::InternalError(_)));
    }

    #[test]
    fn test_chess_error_is_methods() {
        let api_err = ChessError::ApiError("test".to_string());
        assert!(api_err.is_api_error());
        assert!(!api_err.is_parse_error());
        
        let parse_err = ChessError::ParseError("test".to_string());
        assert!(parse_err.is_parse_error());
        assert!(!parse_err.is_api_error());
    }

    #[test]
    fn test_chess_error_message() {
        let err = ChessError::ApiError("test message".to_string());
        assert_eq!(err.message(), "test message");
        
        let err = ChessError::IoError(std::io::Error::new(
            std::io::ErrorKind::Other, "io message"
        ));
        assert_eq!(err.message(), "I/O error");
    }

    #[test]
    fn test_chess_error_convenience_constructors() {
        let err = ChessError::api("api error");
        assert!(matches!(err, ChessError::ApiError(_)));
        
        let err = ChessError::parse("parse error");
        assert!(matches!(err, ChessError::ParseError(_)));
        
        let err = ChessError::network("network error");
        assert!(matches!(err, ChessError::NetworkError(_)));
        
        let err = ChessError::auth("auth error");
        assert!(matches!(err, ChessError::AuthError(_)));
        
        let err = ChessError::validation("validation error");
        assert!(matches!(err, ChessError::ValidationError(_)));
    }
}
