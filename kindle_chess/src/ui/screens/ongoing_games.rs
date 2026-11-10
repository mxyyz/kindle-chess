use std::sync::{Arc, RwLock};

use log::{info, warn};

use crate::i18n::{self, tr};

use super::*;

use crate::ui::layout;
use crate::{
    models::{
        board_api::{GameDataList, PlayedBy},
        chess::ChessApp,
        ui::{Display, Screen, Transition},
    },
    ui::{
        events::{AppEvent, RectangleExt, TouchKind},
        widgets::{Button, IconButton, icon_button::ICON_BUTTON_SIZE},
    },
    utils::config::AppConfig,
};

// ─── ChessGameScreen ──────────────────────────────────────────────────────────

pub struct OngoingChessGamesScreen {
    pub app: ChessApp,
    pub config: Arc<RwLock<AppConfig>>,
    pub prev_page_button: IconButton,
    pub next_page_button: IconButton,
    pub chessgame_button_0: Button,
    pub chessgame_button_1: Button,
    pub chessgame_button_2: Button,
    pub chessgame_button_3: Button,
    pub back_button: IconButton,

    // Async fetch state. `games == None && error == None && !loading` means the
    // screen has not yet kicked off its initial fetch — `render` will trigger it.
    pub games: Option<Arc<GameDataList>>,
    pub error: Option<String>,
    pub loading: bool,

    // Pagination: 4 game buttons per page. Labels are baked into the buttons by
    // `set_page` (called on initial load and on next/prev taps), so `render`
    // doesn't recompute labels every frame.
    pub page_index: usize,
}

impl OngoingChessGamesScreen {
    pub fn new(app: ChessApp, config: Arc<RwLock<AppConfig>>) -> Self {
        // Layout: layout::SCREEN_W × layout::SCREEN_H total canvas
        const BTN_W: u16 = 800;
        const BTN_H: u16 = 120;
        Self {
            config,
            app,
            prev_page_button: IconButton::new(
                layout::CENTER_X - (ICON_BUTTON_SIZE as i16 + 8),
                layout::CENTER_Y * 2 - (ICON_BUTTON_SIZE as i16 - 20) - 32,
                "back-page.png",
            ),
            next_page_button: IconButton::new(
                layout::CENTER_X + 8,
                layout::CENTER_Y * 2 - (ICON_BUTTON_SIZE as i16 - 20) - 32,
                "next-page.png",
            ),
            chessgame_button_0: Button::new(
                layout::CENTER_X - (8 + BTN_W as i16 / 2),
                layout::CENTER_Y - (BTN_H as i16 / 2 + 10),
                BTN_W / 2,
                BTN_H,
                "-".to_string(),
                40.0,
                true,
            ),
            chessgame_button_1: Button::new(
                layout::CENTER_X + 8,
                layout::CENTER_Y - (BTN_H as i16 / 2 + 10),
                BTN_W / 2,
                BTN_H,
                "-".to_string(),
                40.0,
                true,
            ),
            chessgame_button_2: Button::new(
                layout::CENTER_X - (8 + BTN_W as i16 / 2),
                layout::CENTER_Y + (BTN_H as i16 / 2 + 10),
                BTN_W / 2,
                BTN_H,
                "-".to_string(),
                40.0,
                true,
            ),
            chessgame_button_3: Button::new(
                layout::CENTER_X + 8,
                layout::CENTER_Y + (BTN_H as i16 / 2 + 10),
                BTN_W / 2,
                BTN_H,
                String::from("-"),
                40.0,
                true,
            ),
            back_button: IconButton::new(
                32 + ICON_BUTTON_SIZE as i16 / 2,
                layout::CENTER_Y * 2 - (ICON_BUTTON_SIZE as i16 - 20) - 32,
                "back-nav.png",
            ),

            games: None,
            error: None,
            loading: false,
            page_index: 0,
        }
    }
}

// ─── OngoingChessGamesScreen ──────────────────────────────────────────────────────────

const GAMES_PER_PAGE: usize = 4;

impl OngoingChessGamesScreen {
    /// Spawn the ongoing-games fetch onto the tokio runtime. Idempotent w.r.t.
    /// `self.loading` — call freely from `render` (initial load) or a future
    /// reload-button handler. Result is delivered as `OngoingGamesLoaded` /
    /// `OngoingGamesFailed` over the shared event channel.
    fn kick_fetch(&mut self, display: &Display) {
        if self.loading {
            return;
        }
        let Some(api) = self.app.online_idle_api() else {
            self.error = Some("Offline backend has no ongoing games".into());
            return;
        };
        self.loading = true;
        let tx = display.event_tx.clone();
        tokio::spawn(async move {
            match api.get_ongoing_games(8).await {
                Ok(list) => {
                    let _ = tx.send(AppEvent::OngoingGamesLoaded(Arc::new(list)));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::OngoingGamesFailed(e.to_string()));
                }
            }
        });
    }

    fn page_count(&self) -> usize {
        match &self.games {
            Some(g) => g.now_playing.len().div_ceil(GAMES_PER_PAGE),
            None => 0,
        }
    }

    /// Bake labels for `index`'s page of games into the four chess-game buttons,
    /// and update `self.page_index`. No-op when `games` hasn't loaded yet — the
    /// `OngoingGamesLoaded` handler calls this again once data arrives. Slots
    /// past the available game count get a "-" so they're visibly inert; the
    /// touch handler also short-circuits taps on those slots.
    fn set_page(&mut self, index: usize) {
        self.page_index = index;
        let Some(games) = self.games.as_ref() else {
            return;
        };
        let buttons: [&mut Button; 4] = [
            &mut self.chessgame_button_0,
            &mut self.chessgame_button_1,
            &mut self.chessgame_button_2,
            &mut self.chessgame_button_3,
        ];
        let loc = i18n::locale_of(&self.config);
        let start = index * GAMES_PER_PAGE;
        for (k, btn) in buttons.into_iter().enumerate() {
            match games.now_playing.get(start + k) {
                Some(game) => {
                    let opp = match &game.opponent {
                        PlayedBy::User(player) => player.name.clone(),
                        PlayedBy::Ai(computer) => match computer.ai_level {
                            Some(level) => {
                                i18n::t(&loc, "ongoing-ai-level", &[("level", &level.to_string())])
                            }
                            None => tr(&loc, "ongoing-ai"),
                        },
                    };
                    let mut l = i18n::t(&loc, "ongoing-vs", &[("opponent", &opp)]);
                    if game.is_my_turn {
                        l = format!("> {l} <");
                    }
                    btn.label = l;
                }
                None => {
                    btn.label = "-".to_string();
                }
            }
        }
    }

    /// Resolve which game `chessgame_button_{k}` currently maps to, given the
    /// current page. Returns `None` for slots that are inert ("-"-labelled
    /// because the current page has fewer than 4 games left).
    fn game_at_slot(&self, k: usize) -> Option<&crate::models::board_api::GameData> {
        self.games
            .as_ref()?
            .now_playing
            .get(self.page_index * GAMES_PER_PAGE + k)
    }
}

impl Screen for OngoingChessGamesScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        use crate::ui::renderer::DrawColor;

        // First paint after Push: kick off the async fetch. Subsequent renders
        // (after data arrives or on reload) skip this branch.
        if self.games.is_none() && self.error.is_none() && !self.loading {
            self.kick_fetch(display);
        }

        display.renderer.clear(DrawColor::White)?;

        let size_px = 24.0;

        if let Some(err) = &self.error {
            let loc = i18n::locale_of(&self.config);
            let label = i18n::t(&loc, "common-error", &[("error", err)]);
            let (tw, th) = display.renderer.measure_text(&label, size_px);
            let tx = (layout::SCREEN_W - tw as i16) / 2;
            let ty = (layout::SCREEN_H - th as i16) / 2;
            display
                .renderer
                .draw_text(tx, ty, &label, size_px, DrawColor::Black)?;
        } else if self.games.is_some() {
            // Labels were baked into the buttons by `set_page` (called from
            // OngoingGamesLoaded and from next/prev taps), so render is just
            // a draw pass.
            self.chessgame_button_0.draw(&mut display.renderer)?;
            self.chessgame_button_1.draw(&mut display.renderer)?;
            self.chessgame_button_2.draw(&mut display.renderer)?;
            self.chessgame_button_3.draw(&mut display.renderer)?;
            self.back_button.draw(&mut display.renderer)?;
            self.next_page_button.draw(&mut display.renderer)?;
            self.prev_page_button.draw(&mut display.renderer)?;
        } else {
            let label = tr(&i18n::locale_of(&self.config), "common-loading");
            let (tw, th) = display.renderer.measure_text(&label, size_px);
            let tx = (layout::SCREEN_W - tw as i16) / 2;
            let ty = (layout::SCREEN_H - th as i16) / 2;
            display
                .renderer
                .draw_text(tx, ty, &label, size_px, DrawColor::Black)?;
        }

        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        _display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            AppEvent::OngoingGamesLoaded(list) => {
                info!("Ongoing games loaded: {} entries", list.now_playing.len());
                self.games = Some(list);
                self.loading = false;
                // Bake labels for the current page now that we have data.
                self.set_page(self.page_index);
                Ok(Transition::Redraw)
            }

            AppEvent::OngoingGamesFailed(e) => {
                warn!("Ongoing games fetch failed: {}", e);
                self.error = Some(e);
                self.loading = false;
                Ok(Transition::Redraw)
            }

            AppEvent::Touch(touch) => {
                if touch.kind != TouchKind::Up {
                    return Ok(Transition::Stay);
                }

                if self.back_button.rect.contains(touch.x, touch.y) {
                    return Ok(Transition::Pop);
                }

                if self.next_page_button.rect.contains(touch.x, touch.y) {
                    let last = self.page_count().saturating_sub(1);
                    if self.page_index < last {
                        self.set_page(self.page_index + 1);
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }

                if self.prev_page_button.rect.contains(touch.x, touch.y) {
                    if self.page_index > 0 {
                        self.set_page(self.page_index - 1);
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }

                // Game-button taps. Resolve via `game_at_slot` so paginated
                // slots without backing data are inert.
                let game_buttons = [
                    &self.chessgame_button_0,
                    &self.chessgame_button_1,
                    &self.chessgame_button_2,
                    &self.chessgame_button_3,
                ];
                for (k, btn) in game_buttons.into_iter().enumerate() {
                    if !btn.rect.contains(touch.x, touch.y) {
                        continue;
                    }
                    let Some(game) = self.game_at_slot(k) else {
                        return Ok(Transition::Stay);
                    };
                    info!(
                        "Picked ongoing game (page {}, slot {}): {}",
                        self.page_index, k, game.game_id
                    );
                    let game_app = self
                        .app
                        .clone()
                        .attach_game(game.game_id.clone(), game.is_my_turn);
                    return Ok(Transition::Push(Box::new(ChessGameScreen::new(
                        game_app,
                        self.config.clone(),
                    ))));
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

    fn on_reveal(&mut self) {
        // Returning from a game (or any pushed screen): drop the cached list
        // so the next render re-fetches and the ongoing-games view is current.
        self.games = None;
        self.error = None;
    }
}
