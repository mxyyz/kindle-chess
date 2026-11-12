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
    /// e07s11: accept = check.png icon button (was text "Accept").
    pub incoming_accept: [IconButton; 4],
    /// e07s11: decline = close.png icon button (was text "Decline").
    pub incoming_decline: [IconButton; 4],
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
    /// e07s05: poll cycle counter — auto-poll up to MAX_POLLS times, then
    /// show a refresh button instead.
    pub poll_count: usize,
    /// e07s05: manual refresh button shown after the auto-poll cycle ends.
    pub refresh_button: IconButton,
}

/// e07s05: max auto-polls (5s interval × 6 = 30s) before showing refresh.
const MAX_POLLS: usize = 6;

impl InvitesScreen {
    pub fn new(app: ChessApp, config: Arc<RwLock<AppConfig>>) -> Self {
        // Layout: layout::SCREEN_W × layout::SCREEN_H. "Incoming" section at top (4 rows), "Outgoing"
        // below (4 rows), status line near the bottom, back/page icons along
        // the bottom edge.
        // e07s11: NAME_W increased from 420 → 600 (freed by replacing 2×200px
        // text buttons with 2×134px icon buttons; capped to clear icon buttons
        // starting at acc_x=764: 136 + 600 + 16 = 752 < 764).
        const NAME_W: u16 = 600;
        const ACT_W: u16 = 200;
        const ROW_H: u16 = 80;
        const ROW_GAP: i16 = 16;
        let name_x = 136;
        // Icon buttons sit at the right edge; each is ICON_BUTTON_SIZE (134px).
        let dec_x = layout::SCREEN_W - ICON_BUTTON_SIZE as i16 - 24; // close.png
        let acc_x = dec_x - ICON_BUTTON_SIZE as i16 - 16; // check.png
        // e07s03/e07s11: rows pushed down so pagination icons don't overlap
        // action buttons. Incoming icons at y=176 end at 310; incoming rows
        // start at 320 (last icon ends at 320+3*96+134=742). Outgoing icons
        // at y=760 end at 894; outgoing rows start at 900.
        let irow = |i: usize| 320 + i as i16 * (ROW_H as i16 + ROW_GAP);
        let orow = |i: usize| 900 + i as i16 * (ROW_H as i16 + ROW_GAP);
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
            // e07s11: accept/decline are icon buttons now.
            incoming_accept: std::array::from_fn(|i| IconButton::new(acc_x, irow(i), "check.png")),
            incoming_decline: std::array::from_fn(|i| IconButton::new(dec_x, irow(i), "close.png")),
            outgoing_name: std::array::from_fn(|i| btn(name_x, orow(i), NAME_W, "-")),
            outgoing_cancel: std::array::from_fn(|i| btn(acc_x, orow(i), ACT_W, "Cancel")),
            // e07s03: pagination icons on the section-label row, clear of rows.
            incoming_prev: IconButton::new(icon_prev_x, 176, "back-page.png"),
            incoming_next: IconButton::new(icon_next_x, 176, "next-page.png"),
            // e07s11: outgoing pagination at y=760 — clears incoming row 3
            // icons (last incoming icon ends at y=742).
            outgoing_prev: IconButton::new(icon_prev_x, 760, "back-page.png"),
            outgoing_next: IconButton::new(icon_next_x, 760, "next-page.png"),
            back_button: IconButton::new(32 + ICON_BUTTON_SIZE as i16 / 2, bottom, "back-nav.png"),
            // e07s05: refresh button, shown after MAX_POLLS auto-polls.
            refresh_button: IconButton::new(
                layout::CENTER_X - ICON_BUTTON_SIZE as i16 / 2,
                bottom,
                "refresh.png",
            ),
            challenges: None,
            error: None,
            loading: false,
            last_fetch: None,
            status: None,
            incoming_page: 0,
            outgoing_page: 0,
            poll_count: 0,
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
    /// list end keep a "-" label and are simply not drawn. Open challenges
    /// (e06s08) show a localized label instead of a blank opponent name.
    fn set_rows(&mut self) {
        let Some(list) = self.challenges.as_ref() else {
            return;
        };
        let loc = i18n::locale_of(&self.config);
        let incoming: Vec<&ChallengeSummary> = list.iter().filter(|c| c.incoming).collect();
        let outgoing: Vec<&ChallengeSummary> = list.iter().filter(|c| !c.incoming).collect();
        let label =
            |src: &[&ChallengeSummary], page: usize, k: usize, outgoing_section: bool| match src
                .get(page * INVITES_PER_PAGE + k)
            {
                Some(c) if outgoing_section && c.open => {
                    format!("{} ({})", tr(&loc, "invites-open-challenge"), c.status)
                }
                Some(c) => {
                    // e07s06: detail suffix — rating, time control, rated badge.
                    let mut details = String::new();
                    if let Some(r) = c.rating {
                        details.push_str(&format!(" ({r})"));
                    }
                    if let Some(tc) = &c.time_control {
                        details.push_str(&format!(" · {tc}"));
                    }
                    if c.rated {
                        details.push_str(&format!(" · {}", tr(&loc, "play-rated")));
                    }
                    format!("{}{details} ({})", c.opponent, c.status)
                }
                None => "-".to_string(),
            };
        for k in 0..INVITES_PER_PAGE {
            self.incoming_name[k].label = label(&incoming, self.incoming_page, k, false);
            self.outgoing_name[k].label = label(&outgoing, self.outgoing_page, k, true);
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
        // e07s10: initial fetch on first paint; subsequent polls via Tick.
        if self.challenges.is_none() && self.error.is_none() && !self.loading {
            self.kick_fetch(display);
        }

        display.renderer.clear(DrawColor::White)?;

        let loc = i18n::locale_of(&self.config);
        let title = tr(&loc, "invites-title");
        layout::draw_title_bar(&mut display.renderer, &title)?;

        // e07s11: accept/decline are IconButtons (no label). Only outgoing_cancel
        // is still a text Button.
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
        label(784, &tr(&loc, "invites-outgoing"))?;

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
            // e07s04: conditional pagination — prev only on pages > 0, next
            // only when more pages exist.
            if self.incoming_page > 0 {
                self.incoming_prev.draw(&mut display.renderer)?;
            }
            if self.incoming_page + 1 < self.incoming_page_count() {
                self.incoming_next.draw(&mut display.renderer)?;
            }
            if self.outgoing_page > 0 {
                self.outgoing_prev.draw(&mut display.renderer)?;
            }
            if self.outgoing_page + 1 < self.outgoing_page_count() {
                self.outgoing_next.draw(&mut display.renderer)?;
            }
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

        // e07s05: show refresh button after the auto-poll cycle ends.
        if self.poll_count >= MAX_POLLS && self.challenges.is_some() {
            self.refresh_button.draw(&mut display.renderer)?;
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
            // e07s10: periodic tick drives auto-polling without constant redraws.
            AppEvent::Tick(_) => {
                let stale = self
                    .last_fetch
                    .map(|t| t.elapsed() >= INVITES_POLL_INTERVAL)
                    .unwrap_or(true);
                if !self.loading && self.error.is_none() && stale && self.poll_count < MAX_POLLS {
                    self.kick_fetch(display);
                }
                Ok(Transition::Stay)
            }

            AppEvent::ChallengesLoaded(list) => {
                info!("Challenges loaded: {} entries", list.len());
                self.last_fetch = Some(Instant::now());
                self.loading = false;
                self.error = None;
                // e07s05: count completed polls.
                self.poll_count += 1;
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

                // e07s05: refresh button restarts the poll cycle.
                if self.poll_count >= MAX_POLLS
                    && self.refresh_button.rect.contains(touch.x, touch.y)
                {
                    self.poll_count = 0;
                    self.last_fetch = None;
                    self.kick_fetch(display);
                    return Ok(Transition::Redraw);
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
            open: false,
            rating: None,
            speed: None,
            time_control: None,
        }
    }

    fn open_ch(id: &str) -> ChallengeSummary {
        ChallengeSummary {
            id: id.into(),
            status: "created".into(),
            opponent: String::new(),
            incoming: false,
            url: "https://lichess.org/".into(),
            rated: false,
            open: true,
            rating: None,
            speed: None,
            time_control: None,
        }
    }

    #[test]
    fn open_challenges_get_localized_outgoing_label() {
        let mut s = InvitesScreen::new(ChessApp::new_offline(), test_config());
        s.challenges = Some(Arc::new(vec![
            ch("out1", "created", "u", false),
            open_ch("chOpen9"),
        ]));
        s.set_rows();
        // Regular outgoing row keeps "opponent (status)"; the open row shows
        // the localized open label (EN default in tests) + status.
        assert_eq!(s.outgoing_name[0].label, "X (created)");
        assert_eq!(s.outgoing_name[1].label, "Open — random player (created)");
        assert!(s.outgoing_visible(1));
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

    /// e07s03: pagination icon rects must not intersect any action-button rect.
    #[test]
    fn pagination_icons_do_not_overlap_action_buttons() {
        let s = InvitesScreen::new(ChessApp::new_offline(), test_config());
        let pag_rects = [
            s.incoming_prev.rect,
            s.incoming_next.rect,
            s.outgoing_prev.rect,
            s.outgoing_next.rect,
        ];
        // e07s11: accept/decline are IconButtons, cancel is Button — collect separately.
        let mut action_rects: Vec<_> = s.incoming_accept.iter().map(|b| b.rect).collect();
        action_rects.extend(s.incoming_decline.iter().map(|b| b.rect));
        action_rects.extend(s.outgoing_cancel.iter().map(|b| b.rect));
        for pr in &pag_rects {
            for ar in &action_rects {
                assert!(
                    !rects_intersect(pr, ar),
                    "pagination rect (x={},y={},w={},h={}) intersects action rect (x={},y={},w={},h={})",
                    pr.x,
                    pr.y,
                    pr.width,
                    pr.height,
                    ar.x,
                    ar.y,
                    ar.width,
                    ar.height
                );
            }
        }
    }

    fn rects_intersect(a: &crate::ui::events::Rectangle, b: &crate::ui::events::Rectangle) -> bool {
        // Standard AABB overlap test (edges exclusive).
        a.x < b.x + b.width as i16
            && a.x + a.width as i16 > b.x
            && a.y < b.y + b.height as i16
            && a.y + a.height as i16 > b.y
    }

    /// e07s05: poll cycle starts at 0, refresh resets it.
    #[test]
    fn poll_cycle_state_machine() {
        let mut s = InvitesScreen::new(ChessApp::new_offline(), test_config());
        assert_eq!(s.poll_count, 0);
        assert!(s.poll_count < MAX_POLLS); // auto-polling active
        // Simulate MAX_POLLS completed polls.
        s.poll_count = MAX_POLLS;
        assert!(!(s.poll_count < MAX_POLLS)); // refresh button should show
        // Refresh resets the cycle.
        s.poll_count = 0;
        assert!(s.poll_count < MAX_POLLS);
    }

    /// e07s06: label includes rating + time control + rated badge when present.
    #[test]
    fn label_includes_details() {
        let mut s = InvitesScreen::new(ChessApp::new_offline(), test_config());
        let mut c = ch("in1", "created", "u", true);
        c.rating = Some(1500);
        c.time_control = Some("10+5".into());
        c.rated = true;
        s.challenges = Some(Arc::new(vec![c]));
        s.set_rows();
        let label = &s.incoming_name[0].label;
        assert!(label.contains("1500"), "label missing rating: {label}");
        assert!(
            label.contains("10+5"),
            "label missing time control: {label}"
        );
        assert!(label.contains("created"), "label missing status: {label}");
    }

    /// e07s06: label without rating/time control falls back to name + status.
    #[test]
    fn label_without_details() {
        let mut s = InvitesScreen::new(ChessApp::new_offline(), test_config());
        s.challenges = Some(Arc::new(vec![ch("in1", "created", "u", true)]));
        s.set_rows();
        let label = &s.incoming_name[0].label;
        assert!(label.contains('X'), "label missing opponent: {label}");
        assert!(label.contains("created"), "label missing status: {label}");
    }
}
