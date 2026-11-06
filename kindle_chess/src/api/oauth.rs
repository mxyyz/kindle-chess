use crate::{
    api::litchee_adapter::LitcheeAdapter,
    models::oauth::{
        AuthCallbackQuery, AuthConfig, AuthState, LichessUser, OAuth2Client, TokenInfo,
    },
    ui::events::AppEvent,
};
use axum::{
    Router,
    extract::Query,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
};
use image::{ImageBuffer, Luma};
use log::info;
use qrcode::QrCode;
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};
use tower_http::cors::CorsLayer;

use std::fs::{File, remove_file, write};
use std::io::prelude::Read;

impl OAuth2Client {
    pub fn new(config: AuthConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let host_ip = Self::pick_callback_host_ip()?;
        let redirect_uri = format!("http://{}:{}/callback", host_ip, config.redirect_port);
        info!("OAuth redirect URI: {redirect_uri}");

        Ok(Self {
            redirect_url: redirect_uri,
            config,
            state: Arc::new(Mutex::new(None::<AuthState>)),
        })
    }

    /// Picks the IPv4 address the phone must call back on, logging every candidate.
    ///
    /// `local_ip()` returns the *first* non-loopback interface, which is not
    /// necessarily the one the phone can reach: the Kindle can have wifi and the
    /// usbnet gadget up at once (e.g. while plugged in for flashing). Prefer a
    /// wifi-named interface when one exists; log all candidates either way so the
    /// on-device log shows what was available when auth started.
    fn pick_callback_host_ip() -> Result<std::net::IpAddr, Box<dyn std::error::Error>> {
        let candidates: Vec<(String, std::net::IpAddr)> = local_ip_address::list_afinet_netifas()?
            .into_iter()
            .filter(|(name, ip)| ip.is_ipv4() && !ip.is_loopback() && name != "lo")
            .collect();
        info!("Callback host candidates: {:?}", candidates);

        // Kindle wifi interfaces are `wlan*` / `tiwlan*`; anything else (usb0,
        // bridge, docker, …) is a worse guess than the first available address.
        candidates
            .iter()
            .find(|(name, _)| name.starts_with("wlan") || name.starts_with("tiwlan"))
            .or_else(|| candidates.first())
            .map(|(_, ip)| *ip)
            .ok_or_else(|| "no usable IPv4 network interface for OAuth callback".into())
    }

    pub async fn start_auth_flow(&self) -> Result<AuthState, Box<dyn std::error::Error>> {
        // Create a fresh client for this request
        let client = litchee::LichessClient::new();
        let redirect_uri = self.redirect_url.to_string();

        // Get scopes from config and map them to litchee primitives
        // Add mappings here and entries in cfg for future token permissions
        let scopes: Vec<litchee::api::auth::oauth::Scope> = self
            .config
            .scopes
            .iter()
            .filter_map(|s| match s.as_str() {
                "challenge:read" => Some(litchee::api::auth::oauth::Scope::ChallengeRead),
                "challenge:write" => Some(litchee::api::auth::oauth::Scope::ChallengeWrite),
                "follow:read" => Some(litchee::api::auth::oauth::Scope::FollowRead),
                "bot:play" => Some(litchee::api::auth::oauth::Scope::BotPlay),
                "board:play" => Some(litchee::api::auth::oauth::Scope::BoardPlay),
                "puzzle:read" => Some(litchee::api::auth::oauth::Scope::PuzzleRead),
                "puzzle:write" => Some(litchee::api::auth::oauth::Scope::PuzzleWrite),
                _ => None,
            })
            .collect();

        let auth =
            client
                .oauth()
                .authorization_url(&litchee::api::auth::oauth::AuthorizationRequest {
                    client_id: &self.config.client_id,
                    redirect_uri: &redirect_uri,
                    scopes: &scopes,
                    username_hint: None,
                })?;

        // Authorization doesn't derive Clone, but all fields are public —
        // destructure to move them all out at once.
        let litchee::api::auth::oauth::Authorization {
            url,
            state,
            verifier,
        } = auth;

        let auth_state = AuthState {
            state,
            code_verifier: verifier.as_str().to_string(),
            auth_url: url.to_string(),
        };

        // Store state for callback verification
        let mut state_lock = self.state.lock().await;
        *state_lock = Some(auth_state.clone());
        Ok(auth_state)
    }

    pub async fn exchange_code(
        &self,
        code: String,
        state: String,
    ) -> Result<TokenInfo, Box<dyn std::error::Error + Send + Sync>> {
        // Verify state
        let state_lock = self.state.lock().await;
        let stored_state = state_lock.as_ref().ok_or("No auth state found")?;
        if stored_state.state != state {
            return Err("State mismatch - possible CSRF attack".into());
        }

        let verifier = litchee::api::auth::oauth::PkceVerifier::new(&stored_state.code_verifier)?;
        let redirect_uri = self.redirect_url.to_string();
        drop(state_lock);

        // Create a fresh client for token exchange
        let client = litchee::LichessClient::new();
        let token = client
            .oauth()
            .exchange_code(&litchee::api::auth::oauth::CodeExchange {
                code: &code,
                code_verifier: &verifier,
                redirect_uri: &redirect_uri,
                client_id: &self.config.client_id,
            })
            .await?;

        let token_info = TokenInfo {
            access_token: token.access_token.expose().clone(),
            token_type: token.token_type,
            expires_in: Some(token.expires_in as i64),
            scope: None, // litchee's LichessToken doesn't store scopes
        };

        // Clear state after successful exchange
        let mut state_lock = self.state.lock().await;
        *state_lock = None;
        Ok(token_info)
    }
}

pub async fn run_auth_server(
    oauth_client: Arc<OAuth2Client>,
    shutdown_rx: oneshot::Receiver<TokenInfo>,
) -> Result<TokenInfo, Box<dyn std::error::Error>> {
    let (tx, rx) = oneshot::channel::<TokenInfo>();

    let app = Router::new()
        .route(
            "/callback",
            get({
                let tx = Arc::new(Mutex::new(Some(tx)));
                let oauth_client_clone = oauth_client.clone();
                async move |query: Query<AuthCallbackQuery>| {
                    handle_callback(query, oauth_client_clone.clone(), tx.clone()).await
                }
            }),
        )
        .route("/", get(root_handler))
        .layer(CorsLayer::permissive());

    let addr = format!("0.0.0.0:{}", oauth_client.config.redirect_port);
    info!("Starting OAuth callback server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("OAuth callback server listening on {addr}");

    // Run server with graceful shutdown
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        let _ = shutdown_rx.await;
        info!("Shutting down OAuth callback server");
    });

    // Start server in background
    tokio::spawn(async move {
        if let Err(e) = server.await {
            log::error!("Server error: {}", e);
        }
    });

    // Wait for token
    let token = rx.await?;
    Ok(token)
}

async fn handle_callback(
    Query(params): Query<AuthCallbackQuery>,
    oauth_client: Arc<OAuth2Client>,
    tx: Arc<Mutex<Option<oneshot::Sender<TokenInfo>>>>,
) -> axum::response::Response {
    if let Some(error) = params.error {
        let error_msg = format!(
            "Authorization failed: {} - {}",
            error,
            params.error_description.unwrap_or_default()
        );
        info!("{}", error_msg);
        return (
            StatusCode::BAD_REQUEST,
            Html(format!(
                r#"<!DOCTYPE html>
            <html>
            <head><title>Auth Failed</title></head>
            <body>
                <h1>Authorization Failed</h1>
                <p>{}</p>
                <p>You can close this window.</p>
            </body>
            </html>"#,
                error_msg
            )),
        )
            .into_response();
    }

    let code = match params.code {
        Some(c) => c,
        None => {
            info!("No authorization code received");
            return (
                StatusCode::BAD_REQUEST,
                Html(
                    r#"<!DOCTYPE html>
                <html>
                <head><title>Auth Failed</title></head>
                <body>
                    <h1>Authorization Failed</h1>
                    <p>No authorization code received.</p>
                </body>
                </html>"#
                        .to_string(),
                ),
            )
                .into_response();
        }
    };

    let state = match params.state {
        Some(s) => s,
        None => {
            info!("No state parameter received");
            return (
                StatusCode::BAD_REQUEST,
                Html(
                    r#"<!DOCTYPE html>
                <html>
                <head><title>Auth Failed</title></head>
                <body>
                    <h1>Authorization Failed</h1>
                    <p>No state parameter received.</p>
                </body>
                </html>"#
                        .to_string(),
                ),
            )
                .into_response();
        }
    };

    info!("Received authorization code, exchanging for token...");

    match oauth_client.exchange_code(code, state).await {
        Ok(token) => {
            info!("Successfully obtained access token");

            // Send token through channel
            let mut tx_lock = tx.lock().await;
            if let Some(sender) = tx_lock.take() {
                let _ = sender.send(token);
            }

            (
                StatusCode::OK,
                Html(
                    r#"<!DOCTYPE html>
                <html>
                <head>
                    <title>Auth Success</title>
                    <style>
                        body { font-family: Arial; padding: 40px; text-align: center; }
                        .success { color: green; }
                    </style>
                </head>
                <body>
                    <h1 class="success">✓ Authorization Successful!</h1>
                    <p>You have successfully authenticated with Lichess.</p>
                    <p>You can now close this window and return to your application.</p>
                </body>
                </html>"#
                        .to_string(),
                ),
            )
                .into_response()
        }
        Err(e) => {
            info!("Failed to exchange code: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!(
                    r#"<!DOCTYPE html>
                <html>
                <head><title>Auth Failed</title></head>
                <body>
                    <h1>Authorization Failed</h1>
                    <p>Failed to exchange code: {}</p>
                </body>
                </html>"#,
                    e
                )),
            )
                .into_response()
        }
    }
}

async fn root_handler() -> Html<String> {
    Html(
        r#"<!DOCTYPE html>
        <html>
        <head><title>Lichess OAuth</title></head>
        <body>
            <h1>Lichess OAuth Server</h1>
            <p>Waiting for OAuth callback...</p>
        </body>
        </html>"#
            .to_string(),
    )
}

pub fn generate_qr_code(
    url: &str,
) -> Result<ImageBuffer<Luma<u8>, Vec<u8>>, Box<dyn std::error::Error>> {
    let code = QrCode::new(url)?;
    let image = code.render::<Luma<u8>>().build();

    Ok(image)
}

pub async fn start_auth(
    oauth_client: Arc<OAuth2Client>,
) -> Result<(AuthState, ImageBuffer<Luma<u8>, Vec<u8>>), Box<dyn std::error::Error + Send + Sync>> {
    let auth_state = oauth_client.start_auth_flow().await.unwrap();
    info!("Auth URL: {}", auth_state.auth_url);
    let qr = generate_qr_code(&auth_state.auth_url).unwrap();
    Ok((auth_state, qr))
}

pub async fn get_user_info(token: &str) -> Result<LichessUser, Box<dyn std::error::Error>> {
    /*! Tests success of authentication with privileged request (without wrapper) */
    Ok(LitcheeAdapter::new(token)?.get_user_info().await?)
}

pub async fn authenticate(
    tx: std::sync::mpsc::Sender<crate::ui::events::AppEvent>,
) -> Result<(TokenInfo, LichessUser), Box<dyn std::error::Error>> {
    /*! Starts authentication flow */
    let config = AuthConfig::default();
    let oauth_client = Arc::new(OAuth2Client::new(config)?);

    let (_auth_state, qr) = start_auth(oauth_client.clone())
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

    let _ = tx.send(AppEvent::QrReady(qr));

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<TokenInfo>();
    let token = run_auth_server(oauth_client.clone(), shutdown_rx).await?;
    let _ = shutdown_tx.send(token.clone());

    let user = get_user_info(&token.access_token).await?;
    info!("Successfully (re-)authenticated as: {}", user.username);

    match write(env!("AUTH_TOKEN"), serde_json::to_string_pretty(&token)?) {
        Ok(()) => info!("Auth-Token written to {}", env!("AUTH_TOKEN")),
        Err(e) => info!("Error writing AuthToken: {}", e),
    }

    Ok((token, user))
}

pub fn load_token() -> Result<Option<TokenInfo>, Box<dyn std::error::Error>> {
    /*! Loads auth-token from disk */
    let path = std::path::Path::new(env!("AUTH_TOKEN"));

    if !path.exists() {
        return Ok(None);
    }

    let mut file = File::open(path)?;
    let mut buf = vec![];
    file.read_to_end(&mut buf)?;
    // Return None if file is empty -> lets to reauthentication
    if buf.is_empty() {
        return Ok(None);
    }

    let token_info = serde_json::from_slice::<TokenInfo>(&buf[..])?;

    Ok(Some(token_info))
}

pub fn logout() -> std::io::Result<()> {
    remove_file(env!("AUTH_TOKEN"))?;
    Ok(())
}
