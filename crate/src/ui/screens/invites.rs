use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use log::{info, warn};

use crate::i18n::{self, tr};

use super::*;

use crate::ui::layout;
use crate::{
    models::{
        board_api::ChallengeSummary,
        chess::ChessApp,
        ui::{Display, Screen, Transition},
    },
    ui::{
        events::{AppEvent, RectangleExt, TouchKind},
        renderer::DrawColor,
        widgets::{Button, IconButton, icon_button::ICON_BUTTON_SIZE},
    },
    utils::config::AppConfig,
};

// ─── InvitesScreen ────────────────────────────────────────────────────────────
/// Incoming/outgoing friend challenges. Polls the challenge list every 5 s;
/// incoming rows get Accept/Decline, outgoing rows get Cancel. Any challenge
/// whose status flips to "accepted" starts the live game (push ChessGameScreen).
pub struct InvitesScreen {
    pub app: ChessApp,
    pub config: Arc<RwLock<AppConfig>>,
    // Up to 4 incoming + 4 outgoing rows per page: opponent name + action
    // button(s). Rows past the list end are simply not drawn.
    pub incoming_name: [Button; 4],
    pub incoming_accept: [Button; 4],
    pub incoming_decline: [Button; 4],
    pub outgoing_name: [Button; 4],
    pub outgoing_cancel: [Button; 4],
    // Separate pagination per section; each section's page icons sit on its
    // label row, so incoming's controls are above the "Outgoing" label.
    pub incoming_prev: IconButton,
    pub incoming_next: IconButton,
    pub outgoing_prev: IconButton,
    pub outgoing_next: IconButton,
    pub back_button: IconButton,

    pub challenges: Option<Arc<Vec<ChallengeSummary>>>,
    pub error: Option<String>,
    pub loading: bool,
    pub last_fetch: Option<Instant>,
    pub status: Option<String>,
    pub incoming_page: usize,
    pub outgoing_page: usize,
}

impl InvitesScreen {
    pub fn new(app: ChessApp, config: Arc<RwLock<AppConfig>>) -> Self {
        // Layout: layout::SCREEN_W × layout::SCREEN_H. "Incoming" section at top (4 rows), "Outgoing"
        // below (4 rows), status line near the bottom, back/page icons along
        // the bottom edge.
        const NAME_W: u16 = 420;
        const ACT_W: u16 = 200;
        const ROW_H: u16 = 80;
        const ROW_GAP: i16 = 16;
        let name_x = 136;
        let acc_x = 576;
        let dec_x = 792;
        let irow = |i: usize| 240 + i as i16 * (ROW_H as i16 + ROW_GAP);
        let orow = |i: usize| 720 + i as i16 * (ROW_H as i16 + ROW_GAP);
        let btn = |x: i16, y: i16, w: u16, label: &str| {
            Button::new(x, y, w, ROW_H, label.to_string(), 32.0, true)
        };
        let bottom = layout::SCREEN_H - (ICON_BUTTON_SIZE as i16 - 20) - 32;
        // Section label rows: incoming label y=200 (icons y=176), outgoing
        // label y=680 (icons y=656) — icons at the right edge of the canvas.
        let icon_next_x = layout::SCREEN_W - ICON_BUTTON_SIZE as i16 - 24;
        let icon_prev_x = icon_next_x - ICON_BUTTON_SIZE as i16 - 16;
        Self {
            app,
            config,
            incoming_name: std::array::from_fn(|i| btn(name_x, irow(i), NAME_W, "-")),
            incoming_accept: std::array::from_fn(|i| btn(acc_x, irow(i), ACT_W, "Accept")),
            incoming_decline: std::array::from_fn(|i| btn(dec_x, irow(i), ACT_W, "Decline")),
            outgoing_name: std::array::from_fn(|i| btn(name_x, orow(i), NAME_W, "-")),
            outgoing_cancel: std::array::from_fn(|i| btn(acc_x, orow(i), ACT_W, "Cancel")),
            incoming_prev: IconButton::new(icon_prev_x, 176, "back-page.png"),
            incoming_next: IconButton::new(icon_next_x, 176, "next-page.png"),
            outgoing_prev: IconButton::new(icon_prev_x, 656, "back-page.png"),
            outgoing_next: IconButton::new(icon_next_x, 656, "next-page.png"),
            back_button: IconButton::new(32 + ICON_BUTTON_SIZE as i16 / 2, bottom, "back-nav.png"),
            challenges: None,
            error: None,
            loading: false,
            last_fetch: None,
            status: None,
            incoming_page: 0,
            outgoing_page: 0,
        }
    }
}

// ─── InvitesScreen ────────────────────────────────────────────────────────────

/// How often the challenge list is re-polled while the screen is visible.
const INVITES_POLL_INTERVAL: Duration = Duration::from_secs(5);
/// Rows per section (incoming / outgoing) per page.
const INVITES_PER_PAGE: usize = 4;

/// First accepted challenge with a game id — that's a live game to join.
fn accepted_game(challenges: &[ChallengeSummary]) -> Option<&ChallengeSummary> {
    challenges.iter().find(|c| c.game_id().is_some())
}

/// Which challenge action an InvitesScreen row button triggers.
#[derive(Debug, Clone, Copy)]
enum Action {
    Accept,
    Decline,
    Cancel,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Action::Accept => "accept",
            Action::Decline => "decline",
            Action::Cancel => "cancel",
        }
    }
}

impl InvitesScreen {
    /// Spawn the challenge-list fetch. Idempotent w.r.t. `self.loading`.
    fn kick_fetch(&mut self, display: &Display) {
        if self.loading {
            return;
        }
        let Some(api) = self.app.online_idle_api() else {
            self.error = Some("Offline backend has no invites".into());
            return;
        };
        self.loading = true;
        let tx = display.event_tx.clone();
        tokio::spawn(async move {
            match api.list_challenges().await {
                Ok(list) => {
                    let _ = tx.send(AppEvent::ChallengesLoaded(Arc::new(list)));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::ChallengesFailed(e.to_string()));
                }
            }
        });
    }

    /// Spawn an accept/decline/cancel action, then re-fetch the list so the
    /// row disappears on success. The refreshed list arrives as
    /// `ChallengesLoaded`.
    fn kick_action(&mut self, display: &Display, id: String, action: Action) {
        let Some(api) = self.app.online_idle_api() else {
            self.status = Some("Offline backend".into());
            return;
        };
        self.status = Some(format!("Sending {}…", action.label()));
        let tx = display.event_tx.clone();
        tokio::spawn(async move {
            // Map the error to String immediately — `Box<dyn Error>` is not
            // Send, so it must not be held across the next `.await`.
            let res_msg = match action {
                Action::Accept => api.accept_challenge(&id).await.err(),
                Action::Decline => api.decline_challenge(&id).await.err(),
                Action::Cancel => api.cancel_challenge(&id).await.err(),
            }
            .map(|e| format!("{}: {e}", action.label()));
            match api.list_challenges().await {
                Ok(list) => {
                    let _ = tx.send(AppEvent::ChallengesLoaded(Arc::new(list)));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::ChallengesFailed(
                        res_msg.unwrap_or_else(|| e.to_string()),
                    ));
                }
            }
        });
    }

    fn incoming_page_count(&self) -> usize {
        match &self.challenges {
            Some(list) => list
                .iter()
                .filter(|c| c.incoming)
                .count()
                .div_ceil(INVITES_PER_PAGE),
            None => 0,
        }
    }

    fn outgoing_page_count(&self) -> usize {
        match &self.challenges {
            Some(list) => list
                .iter()
                .filter(|c| !c.incoming)
                .count()
                .div_ceil(INVITES_PER_PAGE),
            None => 0,
        }
    }

    /// Bake the current pages' challenges into the 4+4 rows. Slots past the
    /// list end keep a "-" label and are simply not drawn.
    fn set_rows(&mut self) {
        let Some(list) = self.challenges.as_ref() else {
            return;
        };
        let incoming: Vec<&ChallengeSummary> = list.iter().filter(|c| c.incoming).collect();
        let outgoing: Vec<&ChallengeSummary> = list.iter().filter(|c| !c.incoming).collect();
        let label = |src: &[&ChallengeSummary], page: usize, k: usize| match src
            .get(page * INVITES_PER_PAGE + k)
        {
            Some(c) => format!("{} ({})", c.opponent, c.status),
            None => "-".to_string(),
        };
        for k in 0..INVITES_PER_PAGE {
            self.incoming_name[k].label = label(&incoming, self.incoming_page, k);
            self.outgoing_name[k].label = label(&outgoing, self.outgoing_page, k);
        }
    }

    /// Whether row `k` of the section's current page has a backing challenge —
    /// empty rows are not rendered at all.
    fn incoming_visible(&self, k: usize) -> bool {
        self.incoming_at(k).is_some()
    }
    fn outgoing_visible(&self, k: usize) -> bool {
        self.outgoing_at(k).is_some()
    }

    fn incoming_at(&self, k: usize) -> Option<&ChallengeSummary> {
        self.challenges
            .as_ref()?
            .iter()
            .filter(|c| c.incoming)
            .nth(self.incoming_page * INVITES_PER_PAGE + k)
    }

    fn outgoing_at(&self, k: usize) -> Option<&ChallengeSummary> {
        self.challenges
            .as_ref()?
            .iter()
            .filter(|c| !c.incoming)
            .nth(self.outgoing_page * INVITES_PER_PAGE + k)
    }
}

impl Screen for InvitesScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        // Poll: initial fetch, then re-fetch every INVITES_POLL_INTERVAL.
        let stale = self
            .last_fetch
            .map(|t| t.elapsed() >= INVITES_POLL_INTERVAL)
            .unwrap_or(true);
        if !self.loading && self.error.is_none() && stale {
            self.kick_fetch(display);
        }

        display.renderer.clear(DrawColor::White)?;

        let loc = i18n::locale_of(&self.config);
        let title = tr(&loc, "invites-title");
        let (tw, _) = display.renderer.measure_text(&title, 36.0);
        display.renderer.draw_text(
            (layout::SCREEN_W - tw as i16) / 2,
            120,
            &title,
            36.0,
            DrawColor::Black,
        )?;

        for btn in self.incoming_accept.iter_mut() {
            btn.label = tr(&loc, "invites-accept");
        }
        for btn in self.incoming_decline.iter_mut() {
            btn.label = tr(&loc, "invites-decline");
        }
        for btn in self.outgoing_cancel.iter_mut() {
            btn.label = tr(&loc, "invites-cancel");
        }

        let mut label = |y: i16, text: &str| -> Result<(), Box<dyn std::error::Error>> {
            display
                .renderer
                .draw_text(136, y, text, 28.0, DrawColor::Black)?;
            Ok(())
        };
        label(200, &tr(&loc, "invites-incoming"))?;
        label(680, &tr(&loc, "invites-outgoing"))?;

        if let Some(err) = &self.error {
            let label = i18n::t(
                &i18n::locale_of(&self.config),
                "common-error",
                &[("error", err)],
            );
            let (w, h) = display.renderer.measure_text(&label, 24.0);
            display.renderer.draw_text(
                (layout::SCREEN_W - w as i16) / 2,
                (layout::SCREEN_H - h as i16) / 2,
                &label,
                24.0,
                DrawColor::Black,
            )?;
        } else if self.challenges.is_some() {
            // Dynamic rows: only slots backed by a challenge are drawn.
            for k in 0..INVITES_PER_PAGE {
                if self.incoming_visible(k) {
                    self.incoming_name[k].draw(&mut display.renderer)?;
                    self.incoming_accept[k].draw(&mut display.renderer)?;
                    self.incoming_decline[k].draw(&mut display.renderer)?;
                }
                if self.outgoing_visible(k) {
                    self.outgoing_name[k].draw(&mut display.renderer)?;
                    self.outgoing_cancel[k].draw(&mut display.renderer)?;
                }
            }
            self.incoming_prev.draw(&mut display.renderer)?;
            self.incoming_next.draw(&mut display.renderer)?;
            self.outgoing_prev.draw(&mut display.renderer)?;
            self.outgoing_next.draw(&mut display.renderer)?;
        } else {
            let label = tr(&loc, "invites-loading");
            let (w, h) = display.renderer.measure_text(&label, 24.0);
            display.renderer.draw_text(
                (layout::SCREEN_W - w as i16) / 2,
                (layout::SCREEN_H - h as i16) / 2,
                &label,
                24.0,
                DrawColor::Black,
            )?;
        }

        if let Some(status) = &self.status {
            let (w, h) = display.renderer.measure_text(status, 24.0);
            display.renderer.draw_text(
                (layout::SCREEN_W - w as i16) / 2,
                1240 - h as i16 / 2,
                status,
                24.0,
                DrawColor::Black,
            )?;
        }

        self.back_button.draw(&mut display.renderer)?;
        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            AppEvent::ChallengesLoaded(list) => {
                info!("Challenges loaded: {} entries", list.len());
                self.last_fetch = Some(Instant::now());
                self.loading = false;
                self.error = None;
                // An accepted challenge means the game has started — join it.
                if let Some(c) = accepted_game(&list)
                    && let Some(game_id) = c.game_id()
                {
                    let game_id = game_id.to_string();
                    info!("Challenge {} accepted — joining game {}", c.id, game_id);
                    let game_app = self.app.clone().attach_game(game_id, false);
                    return Ok(Transition::Push(Box::new(ChessGameScreen::new(
                        game_app,
                        self.config.clone(),
                    ))));
                }
                self.challenges = Some(list);
                self.set_rows();
                Ok(Transition::Redraw)
            }

            AppEvent::ChallengesFailed(e) => {
                warn!("Challenge list fetch failed: {}", e);
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

                // Per-section pagination: each section pages independently.
                if self.incoming_next.rect.contains(touch.x, touch.y) {
                    let last = self.incoming_page_count().saturating_sub(1);
                    if self.incoming_page < last {
                        self.incoming_page += 1;
                        self.set_rows();
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }
                if self.incoming_prev.rect.contains(touch.x, touch.y) {
                    if self.incoming_page > 0 {
                        self.incoming_page -= 1;
                        self.set_rows();
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }
                if self.outgoing_next.rect.contains(touch.x, touch.y) {
                    let last = self.outgoing_page_count().saturating_sub(1);
                    if self.outgoing_page < last {
                        self.outgoing_page += 1;
                        self.set_rows();
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }
                if self.outgoing_prev.rect.contains(touch.x, touch.y) {
                    if self.outgoing_page > 0 {
                        self.outgoing_page -= 1;
                        self.set_rows();
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }

                // Resolve the tapped row to (challenge id, action) first, so
                // the immutable button borrows end before `kick_action` borrows
                // `self` mutably. Empty slots are inert (not rendered, no hit).
                let mut pending: Option<(String, Action)> = None;
                for (k, btn) in self.incoming_accept.iter().enumerate() {
                    if btn.rect.contains(touch.x, touch.y) {
                        pending = self.incoming_at(k).map(|c| (c.id.clone(), Action::Accept));
                        break;
                    }
                }
                if pending.is_none() {
                    for (k, btn) in self.incoming_decline.iter().enumerate() {
                        if btn.rect.contains(touch.x, touch.y) {
                            pending = self.incoming_at(k).map(|c| (c.id.clone(), Action::Decline));
                            break;
                        }
                    }
                }
                if pending.is_none() {
                    for (k, btn) in self.outgoing_cancel.iter().enumerate() {
                        if btn.rect.contains(touch.x, touch.y) {
                            pending = self.outgoing_at(k).map(|c| (c.id.clone(), Action::Cancel));
                            break;
                        }
                    }
                }
                if let Some((id, action)) = pending {
                    self.kick_action(display, id, action);
                    return Ok(Transition::Redraw);
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
        // Returning to this screen: force a refetch on next render.
        self.last_fetch = None;
        self.challenges = None;
        self.error = None;
    }
}

#[cfg(test)]
mod invites_tests {
    use super::*;
    use crate::models::board_api::ChallengeSummary;
    use crate::ui::screens::test_config;

    fn ch(id: &str, status: &str, url: &str, incoming: bool) -> ChallengeSummary {
        ChallengeSummary {
            id: id.into(),
            status: status.into(),
            opponent: "X".into(),
            incoming,
            url: url.into(),
            rated: false,
        }
    }

    #[test]
    fn accepted_game_finds_only_accepted_with_url() {
        let list = vec![
            ch("a", "created", "https://lichess.org/", true),
            ch("b", "accepted", "https://lichess.org/gameB", false),
            ch("c", "accepted", "https://lichess.org/", true), // empty id segment
        ];
        let found = accepted_game(&list).unwrap();
        assert_eq!(found.id, "b");
        assert_eq!(found.game_id(), Some("gameB"));
    }

    #[test]
    fn no_accepted_means_no_game() {
        let list = vec![ch("a", "created", "https://lichess.org/", true)];
        assert!(accepted_game(&list).is_none());
    }

    #[test]
    fn rows_split_incoming_outgoing_and_hide_empty_slots() {
        let mut s = InvitesScreen::new(ChessApp::new_offline(), test_config());
        s.challenges = Some(Arc::new(vec![
            ch("in1", "created", "u", true),
            ch("out1", "created", "u", false),
            ch("in2", "created", "u", true),
        ]));
        s.set_rows();
        assert_eq!(s.incoming_name[0].label, "X (created)");
        assert_eq!(s.incoming_name[1].label, "X (created)");
        assert!(s.incoming_visible(0));
        assert!(s.incoming_visible(1));
        assert!(!s.incoming_visible(2)); // not rendered
        assert!(s.outgoing_visible(0));
        assert!(!s.outgoing_visible(1));
        assert_eq!(s.incoming_at(0).unwrap().id, "in1");
        assert_eq!(s.incoming_at(2), None);
        assert_eq!(s.outgoing_at(0).unwrap().id, "out1");
        assert_eq!(s.incoming_page_count(), 1);
        assert_eq!(s.outgoing_page_count(), 1);
    }

    #[test]
    fn pagination_is_separate_per_section() {
        let mut s = InvitesScreen::new(ChessApp::new_offline(), test_config());
        let mut list = Vec::new();
        for i in 0..5 {
            list.push(ch(&format!("in{i}"), "created", "u", true));
        }
        for i in 0..2 {
            list.push(ch(&format!("out{i}"), "created", "u", false));
        }
        s.challenges = Some(Arc::new(list));
        // Incoming has 5 -> 2 pages; outgoing has 2 -> 1 page.
        assert_eq!(s.incoming_page_count(), 2);
        assert_eq!(s.outgoing_page_count(), 1);
        // Paging incoming does not touch outgoing's page.
        s.incoming_page = 1;
        s.set_rows();
        assert!(s.incoming_visible(0)); // 5th invite
        assert!(!s.incoming_visible(1));
        assert_eq!(s.incoming_at(0).unwrap().id, "in4");
        assert_eq!(s.outgoing_page, 0);
        assert!(s.outgoing_visible(0));
        assert!(s.outgoing_visible(1));
        assert!(!s.outgoing_visible(2));
    }
}
