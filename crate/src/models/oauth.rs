use std::sync::Arc;
use tokio::sync::Mutex;

use ::uuid::Uuid;
use serde::{Deserialize, Serialize};

// OAUTH2
#[derive(Debug, Deserialize, Serialize)]
pub struct OAuth2Client {
    pub config: AuthConfig,
    pub redirect_url: String,
    #[serde(with = "arc_mutable_option")]
    pub state: Arc<Mutex<Option<AuthState>>>,
}

// Custom serde serialization and deserialization logic for `Arc<Mutex<Option<AuthState>>>`
mod arc_mutable_option {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    use crate::models::oauth::AuthState;

    pub fn serialize<S>(
        _state: &Arc<Mutex<Option<AuthState>>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Implement serialization if needed
        serde::Serialize::serialize(&(), serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Arc<Mutex<Option<AuthState>>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let inner_state: Option<AuthState> = Deserialize::deserialize(deserializer)?;
        Ok(Arc::new(Mutex::new(inner_state)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LichessUser {
    pub id: String,
    pub username: String,
    pub perfs: Option<serde_json::Value>,
    pub created_at: Option<i64>,
    pub disabled: Option<bool>,
    pub tos_violation: Option<bool>,
    pub profile: Option<UserProfile>,
    pub seen_at: Option<i64>,
    pub patron: Option<bool>,
    pub verified: Option<bool>,
    pub play_time: Option<serde_json::Value>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub country: Option<String>,
    pub location: Option<String>,
    pub bio: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub links: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<i64>,
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct AuthConfig {
    pub client_id: String,
    pub redirect_port: u16,
    pub scopes: Vec<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            client_id: format!("lichess-rust-client-{}", Uuid::new_v4()),
            redirect_port: 8080,
            scopes: vec![
                "challenge:read".to_string(),
                "challenge:write".to_string(),
                // Needed for GET /api/rel/following (FriendlistScreen) — without
                // it the endpoint 403s.
                "follow:read".to_string(),
                "bot:play".to_string(),
                "board:play".to_string(),
                // Lets /api/puzzle/next tailor puzzles to the user's rating.
                // Tokens issued before this was added 403 on that path; the
                // puzzle fetch falls back to an anonymous request in that case.
                "puzzle:read".to_string(),
                "puzzle:write".to_string(),
            ],
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AuthState {
    pub state: String,
    pub code_verifier: String,
    pub auth_url: String,
}

#[derive(Debug, Deserialize)]
pub struct AuthCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[cfg(test)]
mod scope_tests {
    use super::*;

    #[test]
    fn default_scopes_include_follow_read_for_friendlist() {
        let scopes = AuthConfig::default().scopes;
        assert!(
            scopes.iter().any(|s| s == "follow:read"),
            "follow:read missing — /api/rel/following would 403"
        );
    }
}

#[cfg(test)]
mod model_tests {
    use super::*;

    // ── TokenInfo JSON round-trip ────────────────────────────────────────────

    #[test]
    fn token_info_roundtrip() {
        let ti = TokenInfo {
            access_token: "tok_abc123".into(),
            token_type: "Bearer".into(),
            expires_in: Some(86400),
            scope: Some("challenge:read board:play".into()),
        };
        let json = serde_json::to_string(&ti).unwrap();
        let back: TokenInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.access_token, "tok_abc123");
        assert_eq!(back.token_type, "Bearer");
        assert_eq!(back.expires_in, Some(86400));
        assert_eq!(back.scope.as_deref(), Some("challenge:read board:play"));
    }

    #[test]
    fn token_info_deserializes_with_optional_fields_absent() {
        let json = r#"{"access_token": "tok", "token_type": "Bearer"}"#;
        let ti: TokenInfo = serde_json::from_str(json).unwrap();
        assert_eq!(ti.access_token, "tok");
        assert_eq!(ti.expires_in, None);
        assert_eq!(ti.scope, None);
    }

    // ── LichessUser with and without profile ─────────────────────────────────

    #[test]
    fn lichess_user_with_profile() {
        let json = r#"{
            "id": "user1",
            "username": "TestUser",
            "profile": {"country": "DE", "bio": "hello", "firstName": "Test"}
        }"#;
        let u: LichessUser = serde_json::from_str(json).unwrap();
        assert_eq!(u.id, "user1");
        assert_eq!(u.username, "TestUser");
        let profile = u.profile.unwrap();
        assert_eq!(profile.country.as_deref(), Some("DE"));
        assert_eq!(profile.bio.as_deref(), Some("hello"));
    }

    #[test]
    fn lichess_user_without_profile() {
        let json = r#"{"id": "user2", "username": "NoProfile"}"#;
        let u: LichessUser = serde_json::from_str(json).unwrap();
        assert_eq!(u.username, "NoProfile");
        assert!(u.profile.is_none());
        assert!(u.perfs.is_none());
        assert!(u.title.is_none());
    }

    #[test]
    fn lichess_user_roundtrip() {
        let u = LichessUser {
            id: "x".into(),
            username: "Y".into(),
            perfs: None,
            created_at: Some(1700000000),
            disabled: None,
            tos_violation: None,
            profile: Some(UserProfile {
                country: Some("US".into()),
                location: None,
                bio: None,
                first_name: None,
                last_name: None,
                links: None,
            }),
            seen_at: None,
            patron: Some(true),
            verified: None,
            play_time: None,
            title: Some("GM".into()),
        };
        let json = serde_json::to_string(&u).unwrap();
        let back: LichessUser = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "x");
        assert_eq!(back.title.as_deref(), Some("GM"));
        assert_eq!(back.patron, Some(true));
    }

    // ── AuthConfig defaults ──────────────────────────────────────────────────

    #[test]
    fn auth_config_default_scopes_include_challenge_and_follow() {
        let cfg = AuthConfig::default();
        assert!(cfg.scopes.iter().any(|s| s == "challenge:read"));
        assert!(cfg.scopes.iter().any(|s| s == "challenge:write"));
        assert!(cfg.scopes.iter().any(|s| s == "follow:read"));
        assert!(cfg.scopes.iter().any(|s| s == "board:play"));
        assert!(cfg.scopes.iter().any(|s| s == "puzzle:read"));
    }

    #[test]
    fn auth_config_default_port_is_8080() {
        assert_eq!(AuthConfig::default().redirect_port, 8080);
    }

    #[test]
    fn auth_config_default_client_id_is_unique() {
        let a = AuthConfig::default().client_id;
        let b = AuthConfig::default().client_id;
        assert_ne!(a, b);
        assert!(a.starts_with("lichess-rust-client-"));
    }

    // ── AuthState fields (PKCE) ──────────────────────────────────────────────

    #[test]
    fn auth_state_carries_pkce_fields() {
        let state = AuthState {
            state: "random_state".into(),
            code_verifier: "verifier_xyz".into(),
            auth_url: "https://lichess.org/oauth?code_challenge=abc&state=random_state".into(),
        };
        assert!(state.auth_url.contains("code_challenge="));
        assert!(state.auth_url.contains("state=random_state"));
        assert!(!state.code_verifier.is_empty());
    }

    // ── AuthCallbackQuery ────────────────────────────────────────────────────

    #[test]
    fn auth_callback_query_deserializes_success_fields() {
        let json = r#"{"code": "abc", "state": "xyz"}"#;
        let q: AuthCallbackQuery = serde_json::from_str(json).unwrap();
        assert_eq!(q.code.as_deref(), Some("abc"));
        assert_eq!(q.state.as_deref(), Some("xyz"));
        assert!(q.error.is_none());
    }

    #[test]
    fn auth_callback_query_deserializes_error_fields() {
        let json = r#"{"error": "access_denied", "error_description": "nope"}"#;
        let q: AuthCallbackQuery = serde_json::from_str(json).unwrap();
        assert_eq!(q.error.as_deref(), Some("access_denied"));
        assert_eq!(q.error_description.as_deref(), Some("nope"));
        assert!(q.code.is_none());
    }
}
