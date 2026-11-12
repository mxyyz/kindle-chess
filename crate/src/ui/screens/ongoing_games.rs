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
        widgets::{IconButton, icon_button::ICON_BUTTON_SIZE},
    },
    utils::config::AppConfig,
};

// ─── ChessGameScreen ──────────────────────────────────────────────────────────

/// e07s11: a square game card — title (vs opponent) + detail rows below.
pub struct GameCard {
    pub rect: crate::ui::events::Rectangle,
    pub title: String,
    pub detail_lines: Vec<String>,
}

pub struct OngoingChessGamesScreen {
    pub app: ChessApp,
    pub config: Arc<RwLock<AppConfig>>,
    pub prev_page_button: IconButton,
    pub next_page_button: IconButton,
    /// e07s11: 4 square game cards (2×2 grid) replacing the old button rows.
    pub game_cards: [GameCard; 4],
    pub back_button: IconButton,

    // Async fetch state. `games == None && error == None && !loading` means the
    // screen has not yet kicked off its initial fetch — `render` will trigger it.
    pub games: Option<Arc<GameDataList>>,
    pub error: Option<String>,
    pub loading: bool,

    // Pagination: 4 game cards per page.
    pub page_index: usize,

    /// e07s05: poll cycle state — auto-poll up to MAX_POLLS_GAMES times.
    pub poll_count: usize,
    pub last_fetch: Option<std::time::Instant>,
    pub refresh_button: IconButton,
}

/// e07s05: poll interval and max auto-polls for ongoing games (5s × 6 = 30s).
const GAMES_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
const MAX_POLLS_GAMES: usize = 6;

impl OngoingChessGamesScreen {
    pub fn new(app: ChessApp, config: Arc<RwLock<AppConfig>>) -> Self {
        // e07s11: 2×2 grid of square cards. Card size 460×460, gap 40.
        // Total grid: 960×960, centered horizontally, starting at y=220.
        const CARD: u16 = 460;
        const GAP: i16 = 40;
        let grid_w = 2 * CARD as i16 + GAP;
        let x0 = (layout::SCREEN_W - grid_w) / 2;
        let y0: i16 = 220;
        let card_rect = |col: usize, row: usize| {
            crate::ui::events::Rectangle::new(
                x0 + col as i16 * (CARD as i16 + GAP),
                y0 + row as i16 * (CARD as i16 + GAP),
                CARD,
                CARD,
            )
        };
        let empty_card = |rect| GameCard {
            rect,
            title: "-".to_string(),
            detail_lines: Vec::new(),
        };
        let bottom = layout::SCREEN_H - (ICON_BUTTON_SIZE as i16 - 20) - 32;
        Self {
            config,
            app,
            prev_page_button: IconButton::new(
                layout::CENTER_X - (ICON_BUTTON_SIZE as i16 + 8),
                bottom,
                "back-page.png",
            ),
            next_page_button: IconButton::new(layout::CENTER_X + 8, bottom, "next-page.png"),
            game_cards: [
                empty_card(card_rect(0, 0)),
                empty_card(card_rect(1, 0)),
                empty_card(card_rect(0, 1)),
                empty_card(card_rect(1, 1)),
            ],
            back_button: IconButton::new(32 + ICON_BUTTON_SIZE as i16 / 2, bottom, "back-nav.png"),
            // e07s05 + review fix: refresh button to the right of the back
            // button (back x=99, width=134 → right edge=233) — clear of both
            // back and the centered pagination buttons.
            refresh_button: IconButton::new(
                32 + ICON_BUTTON_SIZE as i16 / 2 + ICON_BUTTON_SIZE as i16 + 16,
                bottom,
                "refresh.png",
            ),

            games: None,
            error: None,
            loading: false,
            page_index: 0,
            poll_count: 0,
            last_fetch: None,
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

    /// e07s11: bake `index`'s page of games into the 4 square cards.
    /// Each card gets a title ("vs {opponent}") + detail rows (rating, speed,
    /// rated). Empty slots get "-" title and no details.
    fn set_page(&mut self, index: usize) {
        self.page_index = index;
        let Some(games) = self.games.as_ref() else {
            return;
        };
        let loc = i18n::locale_of(&self.config);
        let start = index * GAMES_PER_PAGE;
        for (k, card) in self.game_cards.iter_mut().enumerate() {
            match games.now_playing.get(start + k) {
                Some(game) => {
                    let (opp, opp_rating) = match &game.opponent {
                        PlayedBy::User(player) => (player.name.clone(), Some(player.rating)),
                        PlayedBy::Ai(computer) => match computer.ai_level {
                            Some(level) => (
                                i18n::t(&loc, "ongoing-ai-level", &[("level", &level.to_string())]),
                                None,
                            ),
                            None => (tr(&loc, "ongoing-ai"), None),
                        },
                    };
                    let mut title = format!("vs {opp}");
                    if game.is_my_turn {
                        title = format!("▶ {title}");
                    }
                    card.title = title;
                    card.detail_lines.clear();
                    if let Some(r) = opp_rating {
                        card.detail_lines.push(i18n::t(
                            &loc,
                            "ongoing-rating",
                            &[("rating", &r.to_string())],
                        ));
                    }
                    card.detail_lines.push(i18n::t(
                        &loc,
                        "ongoing-mode",
                        &[("mode", &format!("{:?}", game.speed).to_lowercase())],
                    ));
                    if game.rated {
                        card.detail_lines.push(tr(&loc, "play-rated"));
                    }
                }
                None => {
                    card.title = "-".to_string();
                    card.detail_lines.clear();
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

        // e07s10: initial fetch on first paint (no data yet). Subsequent
        // polls are driven by Tick events in handle_event.
        if self.games.is_none() && self.error.is_none() && !self.loading {
            self.kick_fetch(display);
        }

        display.renderer.clear(DrawColor::White)?;

        // e07s02: uniform title bar.
        let loc = i18n::locale_of(&self.config);
        layout::draw_title_bar(&mut display.renderer, &tr(&loc, "ongoing-title"))?;

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
            // e07s11: draw square game cards — title + detail rows.
            for card in &self.game_cards {
                if card.title == "-" {
                    continue; // empty slot
                }
                // Card border + fill.
                display
                    .renderer
                    .draw_rectangle(card.rect, DrawColor::White, true)?;
                display
                    .renderer
                    .draw_rectangle(card.rect, DrawColor::Black, false)?;
                // Title centered near the top of the card.
                let (tw, th) = display.renderer.measure_text(&card.title, 28.0);
                let tx = card.rect.x + (card.rect.width as i16 - tw as i16) / 2;
                let ty = card.rect.y + 30;
                display
                    .renderer
                    .draw_text(tx, ty, &card.title, 28.0, DrawColor::Black)?;
                let _ = th;
                // Detail rows below the title, one per line, 20px font.
                let mut dy = ty + 50;
                for line in &card.detail_lines {
                    display.renderer.draw_text(
                        card.rect.x + 20,
                        dy,
                        line,
                        20.0,
                        DrawColor::Black,
                    )?;
                    dy += 30;
                }
            }
            self.back_button.draw(&mut display.renderer)?;
            // e07s04: conditional pagination.
            if self.page_index + 1 < self.page_count() {
                self.next_page_button.draw(&mut display.renderer)?;
            }
            if self.page_index > 0 {
                self.prev_page_button.draw(&mut display.renderer)?;
            }
            // e07s05: refresh button after auto-poll cycle ends.
            if self.poll_count >= MAX_POLLS_GAMES {
                self.refresh_button.draw(&mut display.renderer)?;
            }
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
        display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            // e07s10: periodic tick drives auto-polling without constant redraws.
            AppEvent::Tick(_) => {
                let stale = self
                    .last_fetch
                    .map(|t| t.elapsed() >= GAMES_POLL_INTERVAL)
                    .unwrap_or(true);
                if !self.loading
                    && self.error.is_none()
                    && stale
                    && self.poll_count < MAX_POLLS_GAMES
                {
                    self.kick_fetch(display);
                }
                Ok(Transition::Stay)
            }

            AppEvent::OngoingGamesLoaded(list) => {
                info!("Ongoing games loaded: {} entries", list.now_playing.len());
                // e07s10: if current page is empty but data exists, jump to page 0.
                let current_page_empty = self.page_index > 0
                    && list
                        .now_playing
                        .get(self.page_index * GAMES_PER_PAGE)
                        .is_none();
                if current_page_empty && !list.now_playing.is_empty() {
                    self.page_index = 0;
                }
                self.games = Some(list);
                self.loading = false;
                self.last_fetch = Some(std::time::Instant::now());
                // e07s05: count completed polls.
                self.poll_count += 1;
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

                // e07s05: refresh button restarts the poll cycle.
                if self.poll_count >= MAX_POLLS_GAMES
                    && self.refresh_button.rect.contains(touch.x, touch.y)
                {
                    self.poll_count = 0;
                    self.last_fetch = None;
                    self.kick_fetch(display);
                    return Ok(Transition::Redraw);
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

                // e07s11: game-card taps. Resolve via `game_at_slot` so paginated
                // slots without backing data are inert.
                for (k, card) in self.game_cards.iter().enumerate() {
                    if !card.rect.contains(touch.x, touch.y) {
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
        // Review fix: reset poll cycle so auto-polling resumes on re-entry.
        self.poll_count = 0;
        self.last_fetch = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::board_api::{GameData, GameDataList, PlayedBy, PlayedByPlayer};
    use crate::ui::screens::test_config;
    use std::sync::Arc;

    fn dummy_game(id: &str) -> GameData {
        GameData {
            full_id: id.to_string(),
            game_id: id.to_string(),
            color: "white".into(),
            fen: String::new(),
            has_moved: false,
            is_my_turn: true,
            last_move: String::new(),
            opponent: PlayedBy::User(PlayedByPlayer {
                id: "x".into(),
                name: "x".into(),
                title: None,
                rating: 1500,
            }),
            perf: "rapid".into(),
            rated: false,
            seconds_left: None,
            source: "lobby".into(),
            speed: crate::models::board_api::Speed::Rapid,
            variant: crate::models::board_api::GameVariant {
                key: "standard".into(),
                name: "Standard".into(),
                short: None,
            },
        }
    }

    fn make_screen_with(n: usize) -> OngoingChessGamesScreen {
        let mut s = OngoingChessGamesScreen::new(ChessApp::new_offline(), test_config());
        let games: Vec<GameData> = (0..n).map(|i| dummy_game(&format!("g{i}"))).collect();
        s.games = Some(Arc::new(GameDataList { now_playing: games }));
        s
    }

    /// e07s04: page_count edge cases — 0 items, 1 page, 2 pages, exact multiple.
    #[test]
    fn page_count_edge_cases() {
        assert_eq!(make_screen_with(0).page_count(), 0);
        assert_eq!(make_screen_with(1).page_count(), 1);
        assert_eq!(make_screen_with(4).page_count(), 1); // exact multiple of 4
        assert_eq!(make_screen_with(5).page_count(), 2);
        assert_eq!(make_screen_with(8).page_count(), 2); // exact multiple
        assert_eq!(make_screen_with(9).page_count(), 3);
    }

    /// e07s04: conditional pagination — prev hidden on page 0, next hidden on last.
    #[test]
    fn conditional_pagination_visibility() {
        let mut s = make_screen_with(9); // 3 pages
        assert_eq!(s.page_count(), 3);
        // Page 0: prev hidden (page_index == 0), next visible (0+1 < 3).
        assert!(s.page_index == 0);
        assert!(s.page_index + 1 < s.page_count());
        // Last page: next hidden.
        s.page_index = 2;
        assert!(!(s.page_index + 1 < s.page_count()));
        assert!(s.page_index > 0);
    }

    /// Review fix: bottom-bar buttons (back/refresh/prev/next) must not overlap.
    #[test]
    fn bottom_bar_buttons_do_not_overlap() {
        let s = OngoingChessGamesScreen::new(ChessApp::new_offline(), test_config());
        let rects = [
            ("back", s.back_button.rect),
            ("refresh", s.refresh_button.rect),
            ("prev", s.prev_page_button.rect),
            ("next", s.next_page_button.rect),
        ];
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                let (na, ra) = rects[i];
                let (nb, rb) = rects[j];
                let overlap = ra.x < rb.x + rb.width as i16
                    && ra.x + ra.width as i16 > rb.x
                    && ra.y < rb.y + rb.height as i16
                    && ra.y + ra.height as i16 > rb.y;
                assert!(!overlap, "{na} overlaps {nb}");
            }
        }
    }
}
