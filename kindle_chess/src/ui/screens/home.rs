use std::sync::{Arc, RwLock};

use log::{debug, info, warn};

use crate::i18n::{self, tr};

use super::*;

use crate::{
    api::oauth::{get_user_info, load_token},
    models::{
        chess::ChessApp,
        ui::{Display, Screen, Transition},
    },
    ui::{
        events::{AppEvent, RectangleExt, TouchKind},
        renderer::DrawColor,
        widgets::{
            Button,
            icon_button::{Icon, load_icon},
        },
    },
    utils::config::AppConfig,
};

// ─── HomeScreen ───────────────────────────────────────────────────────────────
// The top-level launcher. Add a button here for every future game.

pub struct HomeScreen {
    pub chess_button: Button,
    pub quit_button: Button,
    pub puzzle_button: Button,
    pub ongoing_games_button: Button,
    pub settings_button: Button,
    // App logo, drawn centred in the upper half of the screen. None if the
    // asset failed to load.
    pub logo: Option<Icon>,

    // Auth bootstrap state. `auth_started` flips to true on the first render so
    // we kick the token check exactly once. The chess/ongoing-games buttons
    // stay inert until `app` is populated — either by the bootstrap (token
    // already valid) or by a ChessReady event bubbling up from a popped
    // ChessAuthScreen. The settings button is always live: an offline user
    // still needs to be able to update.
    pub app: Option<ChessApp>,
    pub auth_started: bool,
    /// Shared app configuration, cloned in from `App` at startup. Handed to
    /// the puzzle screen when the Puzzle button is pressed.
    pub config: Arc<RwLock<AppConfig>>,
}

impl HomeScreen {
    pub fn new(config: Arc<RwLock<AppConfig>>) -> Self {
        // Layout: 1072 × 1448 total canvas. The logo occupies the upper half;
        // the five equal buttons are stacked and height-centred in the lower
        // half.
        const BTN_W: u16 = 600;
        const BTN_H: u16 = 96;
        const GAP: i16 = 28;
        const CENTER_X: i16 = 1072 / 2;
        let x = CENTER_X - BTN_W as i16 / 2;
        let total = 5 * BTN_H as i16 + 4 * GAP;
        let upper_half = 1448 / 2;
        let start_y = upper_half + (1448 - upper_half - total) / 2;
        let row = |i: i16| start_y + i * (BTN_H as i16 + GAP);

        Self {
            chess_button: Button::new(x, row(0), BTN_W, BTN_H, String::from("Play"), 45.0, true),
            puzzle_button: Button::new(x, row(1), BTN_W, BTN_H, String::from("Puzzle"), 45.0, true),
            ongoing_games_button: Button::new(
                x,
                row(2),
                BTN_W,
                BTN_H,
                String::from("Ongoing Games"),
                45.0,
                true,
            ),
            settings_button: Button::new(
                x,
                row(3),
                BTN_W,
                BTN_H,
                String::from("Settings"),
                45.0,
                true,
            ),
            quit_button: Button::new(x, row(4), BTN_W, BTN_H, String::from("Quit"), 45.0, true),
            logo: load_home_logo(),
            app: None,
            auth_started: false,
            config,
        }
    }
}

impl Default for HomeScreen {
    fn default() -> Self {
        Self::new(Arc::new(RwLock::new(AppConfig::load())))
    }
}

/// Decode `logo.png` and pre-scale it to its on-screen size — fit within the
/// upper half, aspect preserved. Done once at construction so the home screen
/// doesn't re-downscale a ~1800 px image on every redraw.
fn load_home_logo() -> Option<Icon> {
    let img = load_icon("logo.png")?;
    let lw = img.width() as f32;
    let lh = img.height() as f32;
    let scale = (920.0 / lw).min(570.0 / lh).min(1.0);
    Some(image::imageops::resize(
        &img,
        (lw * scale).max(1.0) as u32,
        (lh * scale).max(1.0) as u32,
        image::imageops::FilterType::Triangle,
    ))
}

// ─── HomeScreen ───────────────────────────────────────────────────────────────

impl Screen for HomeScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        // First render: kick the silent auth bootstrap exactly once. The task
        // posts ChessReady (cached token still valid) or AuthFailed (need QR
        // flow) — both routed back through handle_event.
        if !self.auth_started {
            self.auth_started = true;
            kick_auth_bootstrap(display.event_tx.clone());
        }

        display.renderer.clear(DrawColor::White)?;

        // Localize button labels every render — cheap string lookups, and
        // picks up a language change immediately.
        let loc = i18n::locale_of(&self.config);
        self.chess_button.label = tr(&loc, "home-play");
        self.puzzle_button.label = tr(&loc, "home-puzzle");
        self.ongoing_games_button.label = tr(&loc, "home-ongoing");
        self.settings_button.label = tr(&loc, "home-settings");
        self.quit_button.label = tr(&loc, "home-quit");

        // Logo, centred in the upper third. It was pre-scaled to its display
        // size in HomeScreen::new, so draw it at its own dimensions.
        if let Some(logo) = &self.logo {
            let dw = logo.width() as u16;
            let dh = logo.height() as u16;
            let upper_half = 1448 / 2;
            let lx = (1072 - dw as i16) / 2;
            let ly = (upper_half - dh as i16) / 2;
            display
                .renderer
                .draw_image_alpha(lx, ly, dw, dh, logo, DrawColor::White)?;
        }

        self.chess_button.draw(&mut display.renderer)?;
        self.puzzle_button.draw(&mut display.renderer)?;
        self.ongoing_games_button.draw(&mut display.renderer)?;
        self.settings_button.draw(&mut display.renderer)?;
        self.quit_button.draw(&mut display.renderer)?;

        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        _display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            // Auth completed — either from our own bootstrap, or bubbled up
            // from a popped ChessAuthScreen after the QR flow finished.
            AppEvent::ChessReady(app) => {
                info!("ChessApp ready — buttons live");
                self.app = Some(*app);
                Ok(Transition::Redraw)
            }

            // Bootstrap couldn't authenticate silently. Hand control to the
            // QR-flow screen; on success it'll pop and re-emit ChessReady.
            AppEvent::AuthFailed(e) => {
                warn!("Silent auth failed ({}) — pushing ChessAuthScreen", e);
                Ok(Transition::Push(Box::new(ChessAuthScreen::new(
                    self.config.clone(),
                ))))
            }

            AppEvent::Touch(touch) => {
                if touch.kind == TouchKind::Up {
                    // Settings is reachable regardless of auth state — an
                    // offline user still needs the update entry point.
                    if self.settings_button.rect.contains(touch.x, touch.y) {
                        info!("Settings button pressed");
                        return Ok(Transition::Push(Box::new(SettingsScreen::new(
                            self.config.clone(),
                        ))));
                    }

                    // Quit is always available (even before auth), same as
                    // the global triple-tap gesture.
                    if self.quit_button.rect.contains(touch.x, touch.y) {
                        info!("Quit button pressed");
                        return Ok(Transition::Quit);
                    }

                    // Puzzle is also auth-independent: the daily puzzle needs
                    // no token. A token, if present, is forwarded so the
                    // settings screen can make tailored /puzzle/next requests.
                    if self.puzzle_button.rect.contains(touch.x, touch.y) {
                        info!("Puzzle button pressed");
                        let token = self.app.as_ref().and_then(|a| a.token());
                        return Ok(Transition::Push(Box::new(PuzzleScreen::new(
                            token,
                            self.config.clone(),
                        ))));
                    }

                    let Some(app) = self.app.clone() else {
                        info!("Button tap ignored — auth not yet complete");
                        return Ok(Transition::Stay);
                    };
                    if self.chess_button.rect.contains(touch.x, touch.y) {
                        info!("Play button pressed — pushing PlayScreen");
                        return Ok(Transition::Push(Box::new(PlayScreen::new(
                            app,
                            self.config.clone(),
                        ))));
                    } else if self.ongoing_games_button.rect.contains(touch.x, touch.y) {
                        info!("Ongoing-games button pressed");
                        return Ok(Transition::Push(Box::new(OngoingChessGamesScreen::new(
                            app,
                            self.config.clone(),
                        ))));
                    }
                }

                // A tap that hit no button changes nothing — staying put
                // avoids a full-screen clear+repaint (an e-ink flash).
                Ok(Transition::Stay)
            }

            AppEvent::Expose => {
                debug!("Expose event - redrawing");
                Ok(Transition::Redraw)
            }

            AppEvent::WindowUnmapped => {
                warn!("Window unmapped!");
                Ok(Transition::Stay)
            }

            AppEvent::Quit => {
                info!("Quit requested");
                Ok(Transition::Quit)
            }

            _ => Ok(Transition::Stay),
        }
    }
}

// Spawned from HomeScreen::render on first paint. Tries the cached token,
// validates it via get_user_info, and on success builds the ChessApp.
// Sends ChessReady on success, AuthFailed otherwise — never authenticate()s
// (that's ChessAuthScreen's job, since it owns the QR display).
fn kick_auth_bootstrap(tx: std::sync::mpsc::Sender<AppEvent>) {
    let maybe_token = load_token().map_err(|e| e.to_string());
    tokio::spawn(async move {
        match maybe_token {
            Ok(Some(token_info)) => {
                // Convert the non-Send Box<dyn Error> into a String before the
                // next .await so the future itself stays Send.
                let user_info = get_user_info(&token_info.access_token)
                    .await
                    .map_err(|e| e.to_string());
                match user_info {
                    Ok(user_info) => {
                        info!("Authenticated from cached token as: {}", user_info.username);
                        match ChessApp::new_online(token_info, user_info) {
                            Ok(app) => {
                                let _ = tx.send(AppEvent::ChessReady(Box::new(app)));
                            }
                            Err(e) => {
                                warn!("Failed to build online backend: {} — needs re-auth", e);
                                let _ = tx.send(AppEvent::AuthFailed(e.to_string()));
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Cached token rejected: {} — needs re-auth", e);
                        let _ = tx.send(AppEvent::AuthFailed(e));
                    }
                }
            }
            Ok(None) => {
                info!("No cached token on disk — needs auth");
                let _ = tx.send(AppEvent::AuthFailed("no token".into()));
            }
            Err(e) => {
                let _ = tx.send(AppEvent::AuthFailed(e));
            }
        }
    });
}

#[cfg(test)]
mod home_screen_tests {
    use super::*;
    use crate::ui::screens::test_config;

    #[test]
    fn home_has_five_buttons_all_on_canvas_quit_at_bottom() {
        let s = HomeScreen::new(test_config());
        let buttons = [
            &s.chess_button,
            &s.puzzle_button,
            &s.ongoing_games_button,
            &s.settings_button,
            &s.quit_button,
        ];
        for (i, b) in buttons.iter().enumerate() {
            assert!(
                b.rect.y >= 0 && b.rect.y + b.rect.height as i16 <= 1448,
                "btn {i} off-canvas"
            );
            if i > 0 {
                assert!(
                    b.rect.y > buttons[i - 1].rect.y,
                    "btn {i} not below previous"
                );
            }
        }
        // Quit is the last (bottom-most) button.
        assert!(s.quit_button.rect.y > s.settings_button.rect.y);
    }
}
