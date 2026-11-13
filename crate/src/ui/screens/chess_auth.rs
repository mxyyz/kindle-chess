use std::sync::{Arc, RwLock};

use image::{ImageBuffer, Luma};
use log::{info, warn};

use crate::{
    api::oauth::authenticate,
    models::{
        chess::ChessApp,
        ui::{Display, Screen, Transition},
    },
    ui::{
        events::{AppEvent, Rectangle, RectangleExt},
        renderer::DrawColor,
    },
    utils::config::AppConfig,
};

// ─── ChessAuthScreen ──────────────────────────────────────────────────────────

pub struct ChessAuthScreen {
    pub config: Arc<RwLock<AppConfig>>,
    pub qr_code: Rectangle,
    pub auth_status: Rectangle,
    pub qr_image: Option<ImageBuffer<Luma<u8>, Vec<u8>>>,
    pub auth_url: Option<String>,
    // First-render flag: kick the QR/authenticate flow exactly once.
    pub auth_started: bool,
}

impl ChessAuthScreen {
    pub fn new(config: Arc<RwLock<AppConfig>>) -> Self {
        Self {
            config,
            qr_code: Rectangle::new(286, 400, 500, 500),
            auth_status: Rectangle::new(286, 940, 500, 60),
            qr_image: None,
            auth_url: None,
            auth_started: false,
        }
    }
}

impl Default for ChessAuthScreen {
    fn default() -> Self {
        Self::new(Arc::new(RwLock::new(AppConfig::load())))
    }
}

// ─── ChessAuthScreen ──────────────────────────────────────────────────────────

impl Screen for ChessAuthScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        // Kick the QR/PKCE flow exactly once. authenticate() will post
        // QrReady once the QR image is ready, and AuthSuccess once Lichess
        // redirects to the local callback.
        if !self.auth_started {
            self.auth_started = true;
            let tx = display.event_tx.clone();
            tokio::spawn(async move {
                match authenticate(tx.clone()).await {
                    Ok((token_info, user_info)) => {
                        info!("QR auth succeeded as: {}", user_info.username);
                        let _ = tx.send(AppEvent::AuthSuccess(token_info, user_info));
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::AuthFailed(e.to_string()));
                    }
                }
            });
        }

        display.renderer.clear(DrawColor::White)?;
        // e07s02: uniform title bar.
        {
            let loc = crate::i18n::locale_of(&self.config);
            crate::ui::layout::draw_title_bar(
                &mut display.renderer,
                &crate::i18n::tr(&loc, "auth-title"),
            )?;
        }
        if let Some(ref img) = self.qr_image {
            display.renderer.draw_image(
                self.qr_code.x,
                self.qr_code.y,
                self.qr_code.width,
                self.qr_code.height,
                img,
            )?;
        } else {
            display
                .renderer
                .draw_rectangle(self.qr_code, DrawColor::LightGray, true)?;
        }
        display
            .renderer
            .draw_rectangle(self.auth_status, DrawColor::LightGray, true)?;
        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            AppEvent::AuthSuccess(token, user) => {
                // new_online is sync (no network I/O), but building the Lichess
                // client can still fail — route that back through the auth flow.
                match ChessApp::new_online(token, user) {
                    Ok(app) => {
                        let _ = display.event_tx.send(AppEvent::ChessReady(Box::new(app)));
                    }
                    Err(e) => {
                        warn!("Failed to build online backend: {}", e);
                        let _ = display.event_tx.send(AppEvent::AuthFailed(e.to_string()));
                    }
                }
                Ok(Transition::Stay)
            }

            // Re-emit so HomeScreen (top-of-stack after our Pop) captures the
            // app, then pop ourselves off.
            AppEvent::ChessReady(app) => {
                let _ = display.event_tx.send(AppEvent::ChessReady(app));
                Ok(Transition::Pop)
            }

            AppEvent::QrReady(img) => {
                self.qr_image = Some(img);
                Ok(Transition::Redraw)
            }

            AppEvent::AuthFailed(e) => {
                warn!("QR auth failed: {}", e);
                // TODO: surface error visually + offer retry. For now stay put.
                Ok(Transition::Stay)
            }

            _ => Ok(Transition::Stay),
        }
    }
}
