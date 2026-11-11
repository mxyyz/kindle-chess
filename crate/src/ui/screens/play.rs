use std::sync::{Arc, RwLock};

use log::{info, warn};

use crate::i18n::{self, tr};

use super::*;

use crate::ui::layout;
use crate::{
    models::{
        board_api::{ALLOWED_CORRESPONDENCE_DAYS, ChallengeMode, OpponentKind},
        chess::ChessApp,
        ui::{Display, Screen, Transition},
    },
    ui::{
        events::{AppEvent, Rectangle, RectangleExt, TouchKind},
        renderer::{DrawColor, Renderer},
        widgets::{Button, IconButton, icon_button::ICON_BUTTON_SIZE},
    },
    utils::config::AppConfig,
};

// ─── PlayScreen ───────────────────────────────────────────────────────────────
/// The Play flow's form: pick a game mode (Unlimited / Real time 10:5 /
/// Correspondence with a days toggle), rated/casual, opponent kind
/// (Friend via FriendlistScreen / Computer AI level 1–8 / Random open
/// challenge), then send the challenge.
pub struct PlayScreen {
    pub app: ChessApp,
    pub config: Arc<RwLock<AppConfig>>,
    pub mode_button_unlimited: Button,
    pub mode_button_realtime: Button,
    pub mode_button_correspondence: Button,
    /// Days-per-move toggle group — only drawn in correspondence mode. One
    /// button per Lichess-allowed value (ALLOWED_CORRESPONDENCE_DAYS); a
    /// button group instead of a slider/spinner so rapid taps can't hit the
    /// triple-tap-to-quit gesture and values can't be invalid.
    pub days_buttons: [Button; 7],
    pub selected_days: u8,
    /// Rated/casual toggle (e06) — hidden for Computer (AI games are always
    /// casual on Lichess). Index 0 = casual, 1 = rated.
    pub rated_buttons: [Button; 2],
    /// Opponent-kind selector (e06): 0 = Friend, 1 = Computer, 2 = Random.
    pub kind_buttons: [Button; 3],
    /// AI level row (e06) — only drawn when Computer is selected.
    pub level_buttons: [Button; 8],
    pub opponent_button: Button,
    pub send_button: Button,
    pub invites_button: IconButton,
    pub back_button: IconButton,

    pub selected_mode: ChallengeMode,
    pub opponent: Option<String>,
    /// Index into `kind_buttons`; see `opponent_kind()`.
    pub selected_kind: usize,
    pub rated: bool,
    pub ai_level: u8,
    pub status: Option<String>,
    pub sending: bool,
}

impl PlayScreen {
    pub fn new(app: ChessApp, config: Arc<RwLock<AppConfig>>) -> Self {
        // Layout: layout::SCREEN_W × layout::SCREEN_H. Title top, invites icon top-right, mode row,
        // spinner row, opponent + send buttons, status line, back bottom-left.
        const MODE_W: u16 = 320;
        const MODE_H: u16 = 80;
        const BTN_W: u16 = 600;
        const BTN_H: u16 = 96;
        let mode_x0 = (layout::SCREEN_W - (3 * MODE_W as i16 + 2 * 16)) / 2;
        let x = (layout::SCREEN_W - BTN_W as i16) / 2;
        // Days toggle group: 7 buttons of 100px + 6 gaps of 16 = 796px wide,
        // centred, on the slider's old row (y=310).
        const DAY_W: u16 = 100;
        let days_x0 = (layout::SCREEN_W - (7 * DAY_W as i16 + 6 * 16)) / 2;
        let days_labels = ["1", "2", "3", "5", "7", "10", "14"];
        // e06 rows: rated toggle (y=740), kind selector (y=860), AI level
        // row (y=980) — all above the status line (y=1240).
        const RATED_W: u16 = (BTN_W - 16) / 2;
        const KIND_H: u16 = 80;
        const LEVEL_W: u16 = 100;
        let level_x0 = (layout::SCREEN_W - (8 * LEVEL_W as i16 + 7 * 12)) / 2;
        let bottom = layout::SCREEN_H - (ICON_BUTTON_SIZE as i16 - 20) - 32;
        Self {
            app,
            config,
            mode_button_unlimited: Button::new(
                mode_x0,
                200,
                MODE_W,
                MODE_H,
                "Unlimited".into(),
                32.0,
                true,
            ),
            mode_button_realtime: Button::new(
                mode_x0 + MODE_W as i16 + 16,
                200,
                MODE_W,
                MODE_H,
                "Real time 10:5".into(),
                32.0,
                true,
            ),
            mode_button_correspondence: Button::new(
                mode_x0 + 2 * (MODE_W as i16 + 16),
                200,
                MODE_W,
                MODE_H,
                "Correspondence".into(),
                32.0,
                true,
            ),
            days_buttons: std::array::from_fn(|i| {
                Button::new(
                    days_x0 + i as i16 * (DAY_W as i16 + 16),
                    310,
                    DAY_W,
                    96,
                    days_labels[i].to_string(),
                    32.0,
                    true,
                )
            }),
            selected_days: 3,
            rated_buttons: [
                Button::new(x, 740, RATED_W, BTN_H, "Casual".into(), 36.0, true),
                Button::new(
                    x + RATED_W as i16 + 16,
                    740,
                    RATED_W,
                    BTN_H,
                    "Rated".into(),
                    36.0,
                    true,
                ),
            ],
            kind_buttons: [
                Button::new(mode_x0, 860, MODE_W, KIND_H, "Friend".into(), 32.0, true),
                Button::new(
                    mode_x0 + MODE_W as i16 + 16,
                    860,
                    MODE_W,
                    KIND_H,
                    "Computer".into(),
                    32.0,
                    true,
                ),
                Button::new(
                    mode_x0 + 2 * (MODE_W as i16 + 16),
                    860,
                    MODE_W,
                    KIND_H,
                    "Random".into(),
                    32.0,
                    true,
                ),
            ],
            level_buttons: std::array::from_fn(|i| {
                Button::new(
                    level_x0 + i as i16 * (LEVEL_W as i16 + 12),
                    980,
                    LEVEL_W,
                    KIND_H,
                    (i + 1).to_string(),
                    32.0,
                    true,
                )
            }),
            opponent_button: Button::new(x, 470, BTN_W, BTN_H, "Opponent: -".into(), 40.0, true),
            send_button: Button::new(x, 600, BTN_W, BTN_H, "Send challenge".into(), 40.0, true),
            invites_button: IconButton::new(
                layout::SCREEN_W - ICON_BUTTON_SIZE as i16 - 24,
                24,
                "invitation.png",
            ),
            back_button: IconButton::new(32 + ICON_BUTTON_SIZE as i16 / 2, bottom, "back-nav.png"),
            selected_mode: ChallengeMode::RealTime,
            opponent: None,
            selected_kind: 0,
            rated: false,
            ai_level: 3,
            status: None,
            sending: false,
        }
    }

    /// Label for the opponent button reflecting the current pick.
    pub fn opponent_label(&self) -> String {
        match &self.opponent {
            Some(name) => format!("Opponent: {name}"),
            None => "Opponent: -".to_string(),
        }
    }
}

// ─── PlayScreen ───────────────────────────────────────────────────────────────

impl PlayScreen {
    fn mode(&self) -> ChallengeMode {
        match self.selected_mode {
            ChallengeMode::Correspondence(_) => ChallengeMode::Correspondence(self.selected_days),
            other => other,
        }
    }

    /// Index of the currently selected days button.
    fn selected_days_index(&self) -> usize {
        ALLOWED_CORRESPONDENCE_DAYS
            .iter()
            .position(|&d| d == self.selected_days)
            .unwrap_or(0)
    }

    /// Spawn the challenge send. Result arrives as `ChallengeSent` /
    /// `ChallengeSendFailed` over the event channel.
    fn kick_send(&mut self, display: &Display) {
        let loc = i18n::locale_of(&self.config);
        let Some(opponent) = self.opponent.clone() else {
            self.status = Some(tr(&loc, "play-status-select-first"));
            return;
        };
        let Some(api) = self.app.online_idle_api() else {
            self.status = Some("offline backend".into());
            return;
        };
        if self.sending {
            return;
        }
        self.sending = true;
        self.status = Some(i18n::t(&loc, "play-status-sending", &[("name", &opponent)]));
        let mode = self.mode();
        let tx = display.event_tx.clone();
        tokio::spawn(async move {
            // e06s03 wires the rated toggle / opponent kind here; casual for now.
            match api.send_challenge(&opponent, &mode, false).await {
                Ok(_) => {
                    let _ = tx.send(AppEvent::ChallengeSent(opponent));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::ChallengeSendFailed(e.to_string()));
                }
            }
        });
    }

    /// The correspondence days-spinner is only shown in correspondence mode;
    /// in other modes its row stays blank (layout positions never move).
    fn spinner_visible(&self) -> bool {
        matches!(self.mode(), ChallengeMode::Correspondence(_))
    }

    /// The currently selected mode button, for drawing the selection marker.
    fn selected_mode_button(&self) -> &Button {
        match self.selected_mode {
            ChallengeMode::Unlimited => &self.mode_button_unlimited,
            ChallengeMode::RealTime => &self.mode_button_realtime,
            ChallengeMode::Correspondence(_) => &self.mode_button_correspondence,
        }
    }

    // ── e06: rated toggle + opponent kind + AI level ─────────────────────────

    /// The opponent kind matching the current selector state.
    pub fn opponent_kind(&self) -> OpponentKind {
        match self.selected_kind {
            1 => OpponentKind::Computer(self.ai_level),
            2 => OpponentKind::Random,
            _ => OpponentKind::Friend(self.opponent.clone().unwrap_or_default()),
        }
    }

    pub fn select_kind(&mut self, idx: usize) {
        if idx < self.kind_buttons.len() {
            self.selected_kind = idx;
        }
    }

    pub fn select_rated(&mut self, rated: bool) {
        self.rated = rated;
    }

    /// Sets the AI level; out-of-range values (Lichess allows 1–8) are ignored.
    pub fn select_level(&mut self, level: u8) {
        if let Some(kind) = OpponentKind::computer_level(level) {
            self.ai_level = match kind {
                OpponentKind::Computer(l) => l,
                _ => unreachable!(),
            };
        }
    }

    /// Rated toggle is hidden for Computer — Lichess AI games are always casual.
    fn rated_visible(&self) -> bool {
        self.selected_kind != 1
    }

    /// The AI level row is only shown when Computer is selected.
    fn level_row_visible(&self) -> bool {
        self.selected_kind == 1
    }

    fn selected_rated_button(&self) -> &Button {
        &self.rated_buttons[usize::from(self.rated)]
    }

    fn selected_kind_button(&self) -> &Button {
        &self.kind_buttons[self.selected_kind]
    }
}

/// Selected-file-style marker around `rect`: four 45° diagonal lines from
/// each corner pointing inward — same style as the chessboard's
/// selected-square highlight.
fn draw_selection_marker(
    renderer: &mut Renderer,
    rect: &Rectangle,
) -> Result<(), Box<dyn std::error::Error>> {
    let l = (rect.width.min(rect.height) / 8).max(12) as i16;
    let stroke = 4;
    let x = rect.x;
    let y = rect.y;
    let w = rect.width as i16;
    let h = rect.height as i16;
    let cuts = [
        (x, y, x + l, y + l),
        (x + w, y, x + w - l, y + l),
        (x + w, y + h, x + w - l, y + h - l),
        (x, y + h, x + l, y + h - l),
    ];
    for (x1, y1, x2, y2) in cuts {
        renderer.draw_line(x1, y1, x2, y2, DrawColor::Black, stroke)?;
    }
    Ok(())
}

impl Screen for PlayScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        use crate::ui::renderer::DrawColor;

        display.renderer.clear(DrawColor::White)?;

        let loc = i18n::locale_of(&self.config);
        let title = tr(&loc, "play-title");
        let (tw, _) = display.renderer.measure_text(&title, 36.0);
        display.renderer.draw_text(
            (layout::SCREEN_W - tw as i16) / 2,
            120,
            &title,
            36.0,
            DrawColor::Black,
        )?;

        self.mode_button_unlimited.label = tr(&loc, "play-mode-unlimited");
        self.mode_button_realtime.label = tr(&loc, "play-mode-realtime");
        self.mode_button_correspondence.label = tr(&loc, "play-mode-correspondence");
        self.send_button.label = tr(&loc, "play-send");
        self.rated_buttons[0].label = tr(&loc, "play-casual");
        self.rated_buttons[1].label = tr(&loc, "play-rated");
        self.kind_buttons[0].label = tr(&loc, "play-kind-friend");
        self.kind_buttons[1].label = tr(&loc, "play-kind-computer");
        self.kind_buttons[2].label = tr(&loc, "play-kind-random");

        self.invites_button.draw(&mut display.renderer)?;
        self.mode_button_unlimited.draw(&mut display.renderer)?;
        self.mode_button_realtime.draw(&mut display.renderer)?;
        self.mode_button_correspondence
            .draw(&mut display.renderer)?;
        // Selection marker on the active mode button (selected-file style).
        draw_selection_marker(&mut display.renderer, &self.selected_mode_button().rect)?;

        // Days toggle group — only in correspondence mode; the row stays
        // blank otherwise so nothing below shifts.
        if self.spinner_visible() {
            for btn in &self.days_buttons {
                btn.draw(&mut display.renderer)?;
            }
            draw_selection_marker(
                &mut display.renderer,
                &self.days_buttons[self.selected_days_index()].rect,
            )?;
        }

        // e06: rated toggle (hidden for Computer), kind selector, AI level row.
        if self.rated_visible() {
            for btn in &self.rated_buttons {
                btn.draw(&mut display.renderer)?;
            }
            draw_selection_marker(&mut display.renderer, &self.selected_rated_button().rect)?;
        }
        for btn in &self.kind_buttons {
            btn.draw(&mut display.renderer)?;
        }
        draw_selection_marker(&mut display.renderer, &self.selected_kind_button().rect)?;
        if self.level_row_visible() {
            let level_label = tr(&loc, "play-ai-level");
            display.renderer.draw_text(
                (layout::SCREEN_W - 200) / 2,
                962,
                &level_label,
                24.0,
                DrawColor::Black,
            )?;
            for (i, btn) in self.level_buttons.iter().enumerate() {
                btn.draw(&mut display.renderer)?;
                if i + 1 == self.ai_level as usize {
                    draw_selection_marker(&mut display.renderer, &btn.rect)?;
                }
            }
        }

        self.opponent_button.label = match (self.selected_kind, &self.opponent) {
            (2, _) => tr(&loc, "play-opponent-random"),
            (_, Some(name)) => i18n::t(&loc, "play-opponent", &[("name", name)]),
            (_, None) => tr(&loc, "play-opponent-none"),
        };
        self.opponent_button.draw(&mut display.renderer)?;
        self.send_button.draw(&mut display.renderer)?;

        if let Some(status) = &self.status {
            let (sw, sh) = display.renderer.measure_text(status, 24.0);
            display.renderer.draw_text(
                (layout::SCREEN_W - sw as i16) / 2,
                1240 - sh as i16 / 2,
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
            // Bubbled up from the popped FriendlistScreen.
            AppEvent::FriendSelected(name) => {
                info!("Opponent selected: {}", name);
                self.opponent = Some(name);
                self.status = None;
                Ok(Transition::Redraw)
            }

            AppEvent::ChallengeSent(opponent) => {
                self.sending = false;
                let loc = i18n::locale_of(&self.config);
                self.status = Some(i18n::t(&loc, "play-status-sent", &[("name", &opponent)]));
                // Show the pending outgoing invite on the Invites screen.
                Ok(Transition::Push(Box::new(InvitesScreen::new(
                    self.app.clone(),
                    self.config.clone(),
                ))))
            }

            AppEvent::ChallengeSendFailed(e) => {
                warn!("Challenge send failed: {}", e);
                self.sending = false;
                let loc = i18n::locale_of(&self.config);
                self.status = Some(i18n::t(&loc, "play-status-failed", &[("error", &e)]));
                Ok(Transition::Redraw)
            }

            AppEvent::Touch(touch) => {
                if touch.kind != TouchKind::Up {
                    return Ok(Transition::Stay);
                }

                if self.back_button.rect.contains(touch.x, touch.y) {
                    return Ok(Transition::Pop);
                }

                if self.mode_button_unlimited.rect.contains(touch.x, touch.y) {
                    self.selected_mode = ChallengeMode::Unlimited;
                    return Ok(Transition::Redraw);
                }
                if self.mode_button_realtime.rect.contains(touch.x, touch.y) {
                    self.selected_mode = ChallengeMode::RealTime;
                    return Ok(Transition::Redraw);
                }
                if self
                    .mode_button_correspondence
                    .rect
                    .contains(touch.x, touch.y)
                {
                    self.selected_mode = ChallengeMode::Correspondence(self.selected_days);
                    return Ok(Transition::Redraw);
                }

                // Days toggle group: one button per Lichess-allowed value.
                for (i, btn) in self.days_buttons.iter().enumerate() {
                    if btn.rect.contains(touch.x, touch.y) {
                        self.selected_days = ALLOWED_CORRESPONDENCE_DAYS[i];
                        if matches!(self.selected_mode, ChallengeMode::Correspondence(_)) {
                            self.selected_mode = ChallengeMode::Correspondence(self.selected_days);
                        }
                        return Ok(Transition::Redraw);
                    }
                }

                // e06: rated toggle + kind selector + AI level rows.
                for (i, btn) in self.rated_buttons.iter().enumerate() {
                    if btn.rect.contains(touch.x, touch.y) {
                        self.select_rated(i == 1);
                        return Ok(Transition::Redraw);
                    }
                }
                for (i, btn) in self.kind_buttons.iter().enumerate() {
                    if btn.rect.contains(touch.x, touch.y) {
                        self.select_kind(i);
                        return Ok(Transition::Redraw);
                    }
                }
                for (i, btn) in self.level_buttons.iter().enumerate() {
                    if btn.rect.contains(touch.x, touch.y) {
                        self.select_level((i + 1) as u8);
                        return Ok(Transition::Redraw);
                    }
                }

                // Friend picking still goes through FriendlistScreen; Computer
                // and Random need no opponent name.
                if self.selected_kind == 0 && self.opponent_button.rect.contains(touch.x, touch.y) {
                    info!("Opening friend list");
                    return Ok(Transition::Push(Box::new(FriendlistScreen::new(
                        self.app.clone(),
                        self.config.clone(),
                    ))));
                }

                if self.send_button.rect.contains(touch.x, touch.y) {
                    self.kick_send(display);
                    return Ok(Transition::Redraw);
                }

                if self.invites_button.rect.contains(touch.x, touch.y) {
                    info!("Invites button pressed — pushing InvitesScreen");
                    return Ok(Transition::Push(Box::new(InvitesScreen::new(
                        self.app.clone(),
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
}

#[cfg(test)]
mod play_screen_tests {
    use super::*;
    use crate::models::chess::ChessApp;
    use crate::ui::screens::test_config;

    #[test]
    fn opponent_label_reflects_selection() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        assert_eq!(s.opponent_label(), "Opponent: -");
        s.opponent = Some("Alice42".into());
        assert_eq!(s.opponent_label(), "Opponent: Alice42");
    }

    #[test]
    fn mode_tracks_selected_days_in_correspondence() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        s.selected_mode = ChallengeMode::Correspondence(s.selected_days);
        s.selected_days = 10;
        assert_eq!(s.mode(), ChallengeMode::Correspondence(10));
        s.selected_mode = ChallengeMode::RealTime;
        assert_eq!(s.mode(), ChallengeMode::RealTime);
    }

    #[test]
    fn days_group_only_visible_in_correspondence_mode() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        s.selected_mode = ChallengeMode::RealTime;
        assert!(!s.spinner_visible());
        s.selected_mode = ChallengeMode::Correspondence(3);
        assert!(s.spinner_visible());
        s.selected_mode = ChallengeMode::Unlimited;
        assert!(!s.spinner_visible());
    }

    #[test]
    fn correspondence_days_are_always_lichess_allowed() {
        let s = PlayScreen::new(ChessApp::new_offline(), test_config());
        // Default and every selectable button map to an allowed value.
        assert!(ALLOWED_CORRESPONDENCE_DAYS.contains(&s.selected_days));
        for (i, btn) in s.days_buttons.iter().enumerate() {
            assert_eq!(
                btn.label.parse::<u8>().unwrap(),
                ALLOWED_CORRESPONDENCE_DAYS[i]
            );
        }
        // Selected index tracks selected_days.
        let mut s2 = PlayScreen::new(ChessApp::new_offline(), test_config());
        s2.selected_days = 14;
        assert_eq!(s2.selected_days_index(), 6);
        assert_eq!(s2.mode().days_value(), 0); // mode is RealTime by default
        s2.selected_mode = ChallengeMode::Correspondence(14);
        assert_eq!(s2.mode().days_value(), 14);
    }

    #[test]
    fn selected_mode_button_tracks_selection() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        s.selected_mode = ChallengeMode::Unlimited;
        assert!(std::ptr::eq(
            s.selected_mode_button(),
            &s.mode_button_unlimited
        ));
        s.selected_mode = ChallengeMode::Correspondence(3);
        assert!(std::ptr::eq(
            s.selected_mode_button(),
            &s.mode_button_correspondence
        ));
    }

    // ── e06s02: rated toggle + opponent kind + AI level ──────────────────────

    use crate::models::board_api::OpponentKind;

    fn rects_overlap(a: &Rectangle, b: &Rectangle) -> bool {
        a.x < b.x + b.width as i16
            && b.x < a.x + a.width as i16
            && a.y < b.y + b.height as i16
            && b.y < a.y + a.height as i16
    }

    #[test]
    fn e06_buttons_fit_screen_and_do_not_overlap() {
        let s = PlayScreen::new(ChessApp::new_offline(), test_config());
        let mut rects: Vec<&Rectangle> = vec![
            &s.mode_button_unlimited.rect,
            &s.mode_button_realtime.rect,
            &s.mode_button_correspondence.rect,
            &s.opponent_button.rect,
            &s.send_button.rect,
            &s.invites_button.rect,
            &s.back_button.rect,
        ];
        for b in &s.days_buttons {
            rects.push(&b.rect);
        }
        for b in &s.rated_buttons {
            rects.push(&b.rect);
        }
        for b in &s.kind_buttons {
            rects.push(&b.rect);
        }
        for b in &s.level_buttons {
            rects.push(&b.rect);
        }
        for r in &rects {
            assert!(r.x >= 0 && r.y >= 0, "negative origin: {r:?}");
            assert!(
                r.x + r.width as i16 <= layout::SCREEN_W,
                "overflows right: {r:?}"
            );
            assert!(
                r.y + r.height as i16 <= layout::SCREEN_H,
                "overflows bottom: {r:?}"
            );
        }
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                assert!(
                    !rects_overlap(rects[i], rects[j]),
                    "overlap: {:?} vs {:?}",
                    rects[i],
                    rects[j]
                );
            }
        }
    }

    #[test]
    fn e06_defaults_are_casual_friend_realtime_level_3() {
        let s = PlayScreen::new(ChessApp::new_offline(), test_config());
        assert!(!s.rated);
        assert_eq!(s.selected_kind, 0);
        assert_eq!(s.ai_level, 3);
        assert_eq!(s.opponent_kind(), OpponentKind::Friend(String::new()));
    }

    #[test]
    fn e06_opponent_kind_mapping() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        s.opponent = Some("Alice".into());
        s.select_kind(0);
        assert_eq!(s.opponent_kind(), OpponentKind::Friend("Alice".into()));
        s.select_kind(1);
        s.select_level(7);
        assert_eq!(s.opponent_kind(), OpponentKind::Computer(7));
        s.select_kind(2);
        assert_eq!(s.opponent_kind(), OpponentKind::Random);
    }

    #[test]
    fn e06_rated_toggle_and_visibility_rules() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        assert!(s.rated_visible()); // Friend: rated toggle shown
        s.select_rated(true);
        assert!(s.rated);
        s.select_kind(1); // Computer: AI is always casual -> toggle hidden
        assert!(!s.rated_visible());
        assert!(s.level_row_visible());
        s.select_kind(2); // Random: open challenge can be rated
        assert!(s.rated_visible());
        assert!(!s.level_row_visible());
    }

    #[test]
    fn e06_select_level_rejects_out_of_range() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        s.select_level(0);
        assert_eq!(s.ai_level, 3, "0 is invalid, keep previous");
        s.select_level(9);
        assert_eq!(s.ai_level, 3, "9 is invalid, keep previous");
        s.select_level(8);
        assert_eq!(s.ai_level, 8);
    }

    // ── e06s03: send-flow routing seams ──────────────────────────────────────

    #[test]
    fn e06s03_send_status_key_per_kind() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        s.select_kind(0);
        assert_eq!(s.send_status_key(), "play-status-sending");
        s.select_kind(1);
        assert_eq!(s.send_status_key(), "play-status-sending-ai");
        s.select_kind(2);
        assert_eq!(s.send_status_key(), "play-status-sending-open");
    }

    #[test]
    fn e06s03_friend_without_opponent_needs_selection_first() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        s.select_kind(0);
        s.opponent = None;
        assert!(s.send_requires_opponent());
        s.opponent = Some("Alice".into());
        assert!(!s.send_requires_opponent());
        s.select_kind(1); // Computer: no opponent name needed
        assert!(!s.send_requires_opponent());
        s.select_kind(2); // Random: open challenge, no opponent name needed
        assert!(!s.send_requires_opponent());
    }

    #[test]
    fn e06s03_sent_status_key_for_random_open_challenge() {
        // Friend send carries the opponent name; Random carries an empty name
        // and must show the "open challenge" status instead.
        assert_eq!(PlayScreen::sent_status_key("Alice"), "play-status-sent");
        assert_eq!(PlayScreen::sent_status_key(""), "play-status-open-sent");
    }

    #[test]
    fn e06s03_send_event_per_kind() {
        let friend = PlayScreen::send_event(&OpponentKind::Friend("Alice".into()), None);
        assert!(matches!(friend, AppEvent::ChallengeSent(ref n) if n == "Alice"));
        let open = PlayScreen::send_event(&OpponentKind::Random, None);
        assert!(matches!(open, AppEvent::ChallengeSent(ref n) if n.is_empty()));
        let ai = PlayScreen::send_event(&OpponentKind::Computer(5), Some("g42".into()));
        assert!(
            matches!(ai, AppEvent::AiGameStarted { ref game_id, my_turn: true } if game_id == "g42")
        );
    }
}
