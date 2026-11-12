use std::sync::{Arc, RwLock};

use log::{info, warn};

use crate::i18n::{self, tr};

use crate::ui::layout;
use crate::{
    models::{
        chess::ChessApp,
        ui::{Display, Screen, Transition},
    },
    ui::{
        events::{AppEvent, RectangleExt, TouchKind},
        renderer::DrawColor,
        widgets::Button,
    },
    utils::config::AppConfig,
};

// ─── GameActionsScreen ────────────────────────────────────────────────────────
// Reached from the live-game sidebar's settings button. Hosts the irreversible
// game actions — resign and abort — that aren't part of normal board play.

pub struct GameActionsScreen {
    pub app: ChessApp,
    pub config: Arc<RwLock<AppConfig>>,
    pub resign_button: Button,
    pub abort_button: Button,
    pub back_button: Button,
}

impl GameActionsScreen {
    pub fn new(app: ChessApp, config: Arc<RwLock<AppConfig>>) -> Self {
        const BTN_W: u16 = 600;
        const BTN_H: u16 = 120;
        let x = layout::CENTER_X - BTN_W as i16 / 2;
        Self {
            config,
            app,
            resign_button: Button::new(
                x,
                layout::CENTER_Y - BTN_H as i16 - 10,
                BTN_W,
                BTN_H,
                String::from("Resign"),
                40.0,
                true,
            ),
            abort_button: Button::new(
                x,
                layout::CENTER_Y + 10,
                BTN_W,
                BTN_H,
                String::from("Abort"),
                40.0,
                true,
            ),
            back_button: Button::new(
                x,
                layout::CENTER_Y * 2 - (BTN_H as i16 - 20) - 32,
                BTN_W,
                BTN_H - 20,
                String::from("back"),
                40.0,
                true,
            ),
        }
    }
}

// ─── GameActionsScreen ────────────────────────────────────────────────────────

impl Screen for GameActionsScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        display.renderer.clear(DrawColor::White)?;

        let loc = i18n::locale_of(&self.config);
        self.resign_button.label = tr(&loc, "game-resign");
        self.abort_button.label = tr(&loc, "game-abort");
        self.back_button.label = tr(&loc, "settings-back");
        let title = tr(&loc, "game-actions-title");
        layout::draw_page_title(&mut display.renderer, &title)?;

        self.resign_button.draw(&mut display.renderer)?;
        self.abort_button.draw(&mut display.renderer)?;
        self.back_button.draw(&mut display.renderer)?;

        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        _display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            AppEvent::Touch(touch) => {
                if touch.kind != TouchKind::Up {
                    return Ok(Transition::Stay);
                }

                // Resign / abort fire-and-forget — the game-state stream still
                // running on ChessGameScreen picks up the terminal state and
                // flips the sidebar to "game over" once we pop back to it.
                if self.resign_button.rect.contains(touch.x, touch.y) {
                    if let Some(api) = self.app.online_in_game_api() {
                        info!("Resigning game");
                        tokio::spawn(async move {
                            if let Err(e) = api.resign_game().await {
                                warn!("resign_game failed: {}", e);
                            }
                        });
                    } else {
                        warn!("Resign with no in-game backend — ignored");
                    }
                    return Ok(Transition::Pop);
                }
                if self.abort_button.rect.contains(touch.x, touch.y) {
                    if let Some(api) = self.app.online_in_game_api() {
                        info!("Aborting game");
                        tokio::spawn(async move {
                            if let Err(e) = api.abort_game().await {
                                warn!("abort_game failed: {}", e);
                            }
                        });
                    } else {
                        warn!("Abort with no in-game backend — ignored");
                    }
                    return Ok(Transition::Pop);
                }
                if self.back_button.rect.contains(touch.x, touch.y) {
                    return Ok(Transition::Pop);
                }
                Ok(Transition::Stay)
            }

            AppEvent::Expose => Ok(Transition::Redraw),

            AppEvent::WindowUnmapped => {
                warn!("Window unmapped!");
                Ok(Transition::Stay)
            }

            AppEvent::Quit => Ok(Transition::Quit),

            _ => Ok(Transition::Stay),
        }
    }
}
