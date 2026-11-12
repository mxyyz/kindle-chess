use std::sync::{Arc, RwLock};

use log::{info, warn};

use crate::i18n::{self, tr};

use super::*;

use crate::ui::layout;
use crate::{
    models::{
        board_api::{ALLOWED_CORRESPONDENCE_DAYS, ChallengeMode, OpponentKind, RematchContext},
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

// e06s07 layout rows (user-approved design 2026-11-11). Rows 1–4 live in the
// upper half (y < SCREEN_H/2 = 724); Send is centered in the lower half.
/// Row 1: opponent-kind group.
const KIND_Y: i16 = 200;
const KIND_H: u16 = 80;
/// Row 2: shared selector row (AI level | days | random info) — exactly one.
/// Positioned so the margin above the row equals the margin below it (the
/// kind row ends at 280, the mode row starts at 470; the 96px selector row
/// splits the remaining space evenly -> y=327, 47px gaps on both sides).
/// The 47px upper gap fits the 24px "AI level" label (30px tall) with its
/// 10px margin to the row, clear of the kind row (BUG fix 2026-11-11: the
/// label used to fill the old 30px gap and touch the Friend button).
const SELECTOR_Y: i16 =
    KIND_Y + KIND_H as i16 + (MODE_Y - KIND_Y - KIND_H as i16 - SELECTOR_H as i16) / 2;
const SELECTOR_H: u16 = 96;
/// Margin between the selector row and its labels (AI-level above, days below).
const SELECTOR_LABEL_MARGIN: i16 = 10;
/// Row 3: game mode group.
const MODE_Y: i16 = 470;
const MODE_H: u16 = 80;
/// Row 4: rated toggle — last row of the upper half (hidden for Computer).
const RATED_Y: i16 = 580;
/// Send button, centered in the lower half.
const SEND_Y: i16 = 1038;
/// Selector-row x origins (both groups are centered on the same row).
const LEVEL_X0: i16 = (layout::SCREEN_W - (8 * 100 + 7 * 12)) / 2; // 8 × 100px + 7 × 12
const DAYS_X0: i16 = (layout::SCREEN_W - (7 * 100 + 6 * 16)) / 2; // 7 × 100px + 6 × 16

/// kind_buttons indices (e06s07): Computer | Friend | Random.
const KIND_COMPUTER: usize = 0;
const KIND_FRIEND: usize = 1;
const KIND_RANDOM: usize = 2;

/// What the shared selector row (row 2) currently hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectorMode {
    /// AI level 1–8 button row (kind = Computer).
    AiLevel,
    /// Days-per-move button row (Correspondence and kind ≠ Computer).
    Days,
    /// Fallback slot: open-challenge info text for Random, blank row for
    /// Friend (2026-11-11 adjustment).
    RandomInfo,
}

/// Which selector was selected last — when Computer AND Correspondence are
/// both active, the shared row renders the last-selected one (user design
/// 2026-11-11), not a fixed priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorFocus {
    Level,
    Days,
}

/// The Play flow's form (e06s07 layout): row 1 opponent kind
/// (Computer / Friend / Random — Friend also opens the FriendlistScreen and
/// then shows the picked name), row 2 shared selector (level | days | info),
/// row 3 game mode, row 4 rated/casual (hidden for Computer), Send centered
/// in the lower half.
pub struct PlayScreen {
    pub app: ChessApp,
    pub config: Arc<RwLock<AppConfig>>,
    pub mode_button_unlimited: Button,
    pub mode_button_realtime: Button,
    pub mode_button_correspondence: Button,
    /// Days-per-move toggle group — drawn in the shared selector row when
    /// mode = Correspondence and kind ≠ Computer. One button per
    /// Lichess-allowed value (ALLOWED_CORRESPONDENCE_DAYS); a button group
    /// instead of a slider/spinner so rapid taps can't hit the
    /// triple-tap-to-quit gesture and values can't be invalid.
    pub days_buttons: [Button; 7],
    pub selected_days: u8,
    /// Rated/casual toggle (e06s07 row 4) — hidden for Computer (AI games are
    /// always casual on Lichess). Index 0 = casual, 1 = rated.
    pub rated_buttons: [Button; 2],
    /// Opponent-kind selector (e06s07 row 1): Computer | Friend | Random.
    pub kind_buttons: [Button; 3],
    /// AI level row (e06s07 selector row) — drawn when Computer is selected.
    pub level_buttons: [Button; 8],
    pub send_button: Button,
    pub invites_button: IconButton,
    pub back_button: IconButton,

    pub selected_mode: ChallengeMode,
    pub opponent: Option<String>,
    /// Index into `kind_buttons`; see `opponent_kind()`.
    pub selected_kind: usize,
    /// Last-selected selector (level vs days) — decides the shared row when
    /// Computer and Correspondence are both active.
    pub selector_focus: SelectorFocus,
    pub rated: bool,
    pub ai_level: u8,
    pub status: Option<String>,
    pub sending: bool,
    /// e07s01: modal list-picker overlay for choosing a friend opponent.
    /// When `Some`, it captures all touch input and renders on top.
    pub list_picker: Option<crate::ui::widgets::ListPicker>,
    /// Cached friend list for the picker (fetched async on Friend tap).
    pub friends: Option<Arc<Vec<String>>>,
    pub friends_loading: bool,
    /// When true, the next render skips the full-screen `clear(White)`.
    /// Set on selection-only changes (mode/kind/rated/level/days taps) so
    /// the e-ink panel doesn't flash — buttons draw their own white bg,
    /// so overdraw covers the previous content without a global clear.
    pub skip_full_clear: bool,
}

impl PlayScreen {
    pub fn new(app: ChessApp, config: Arc<RwLock<AppConfig>>) -> Self {
        // e06s07 layout (layout::SCREEN_W × layout::SCREEN_H = 1072×1448):
        //   row 1  y=200 h=80   kind group Computer | Friend | Random
        //   row 2  y=310 h=96   shared selector (level | days | info)
        //   row 3  y=470 h=80   mode group
        //   row 4  y=580 h=96   rated toggle (last upper-half row, ends 676 < 724)
        //   lower half          Send centered (y=1038, x=236, 600×96)
        // Title top, invites icon top-right, status line y=1240, back bottom-left.
        const MODE_W: u16 = 320;
        const BTN_W: u16 = 600;
        const BTN_H: u16 = 96;
        let mode_x0 = (layout::SCREEN_W - (3 * MODE_W as i16 + 2 * 16)) / 2;
        let x = (layout::SCREEN_W - BTN_W as i16) / 2;
        // Selector row: days group 7 × 100px + 6 gaps of 16, AI level group
        // 8 × 100px + 7 gaps of 12 — both centered on the same row (y=310);
        // only one is drawn at a time.
        const DAY_W: u16 = 100;
        let days_x0 = DAYS_X0;
        let days_labels = ["1", "2", "3", "5", "7", "10", "14"];
        const RATED_W: u16 = (BTN_W - 16) / 2;
        const LEVEL_W: u16 = 100;
        let level_x0 = LEVEL_X0;
        let bottom = layout::SCREEN_H - (ICON_BUTTON_SIZE as i16 - 20) - 32;
        Self {
            app,
            config,
            mode_button_unlimited: Button::new(
                mode_x0,
                MODE_Y,
                MODE_W,
                MODE_H,
                "Unlimited".into(),
                32.0,
                true,
            ),
            mode_button_realtime: Button::new(
                mode_x0 + MODE_W as i16 + 16,
                MODE_Y,
                MODE_W,
                MODE_H,
                "Real time 10:5".into(),
                32.0,
                true,
            ),
            mode_button_correspondence: Button::new(
                mode_x0 + 2 * (MODE_W as i16 + 16),
                MODE_Y,
                MODE_W,
                MODE_H,
                "Correspondence".into(),
                32.0,
                true,
            ),
            days_buttons: std::array::from_fn(|i| {
                Button::new(
                    days_x0 + i as i16 * (DAY_W as i16 + 16),
                    SELECTOR_Y,
                    DAY_W,
                    SELECTOR_H,
                    days_labels[i].to_string(),
                    32.0,
                    true,
                )
            }),
            selected_days: 3,
            rated_buttons: [
                Button::new(x, RATED_Y, RATED_W, BTN_H, "Casual".into(), 36.0, true),
                Button::new(
                    x + RATED_W as i16 + 16,
                    RATED_Y,
                    RATED_W,
                    BTN_H,
                    "Rated".into(),
                    36.0,
                    true,
                ),
            ],
            kind_buttons: [
                Button::new(
                    mode_x0,
                    KIND_Y,
                    MODE_W,
                    KIND_H,
                    "Computer".into(),
                    32.0,
                    true,
                ),
                Button::new(
                    mode_x0 + MODE_W as i16 + 16,
                    KIND_Y,
                    MODE_W,
                    KIND_H,
                    "Friend".into(),
                    32.0,
                    true,
                ),
                Button::new(
                    mode_x0 + 2 * (MODE_W as i16 + 16),
                    KIND_Y,
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
                    SELECTOR_Y,
                    LEVEL_W,
                    SELECTOR_H,
                    (i + 1).to_string(),
                    32.0,
                    true,
                )
            }),
            send_button: Button::new(x, SEND_Y, BTN_W, BTN_H, "Send challenge".into(), 40.0, true),
            invites_button: IconButton::new(
                layout::SCREEN_W - ICON_BUTTON_SIZE as i16 - 24,
                24,
                "invitation.png",
            ),
            back_button: IconButton::new(32 + ICON_BUTTON_SIZE as i16 / 2, bottom, "back-nav.png"),
            selected_mode: ChallengeMode::RealTime,
            opponent: None,
            selected_kind: KIND_FRIEND,
            selector_focus: SelectorFocus::Level,
            rated: false,
            ai_level: 3,
            status: None,
            sending: false,
            list_picker: None,
            friends: None,
            friends_loading: false,
            skip_full_clear: false,
        }
    }

    /// Label shown on the Friend kind button: the picked friend's name, or
    /// the localized default. Long names get a smaller font so the label
    /// stays inside the button (e06s07).
    pub fn friend_button_label(&self) -> (String, f32) {
        match &self.opponent {
            Some(name) => (name.clone(), friend_label_font(name.len())),
            None => ("Friend".to_string(), 32.0),
        }
    }
}

/// Font size for the Friend kind button so long usernames still fit.
fn friend_label_font(len: usize) -> f32 {
    match len {
        0..=10 => 32.0,
        11..=16 => 26.0,
        _ => 20.0,
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

    // ── e06s03: send-flow routing seams ─────────────────────────────────────

    /// i18n key for the "sending" status, per opponent kind.
    fn send_status_key(&self) -> &'static str {
        match self.selected_kind {
            KIND_COMPUTER => "play-status-sending-ai",
            KIND_RANDOM => "play-status-sending-open",
            _ => "play-status-sending",
        }
    }

    /// Whether the current kind needs a picked opponent name before sending.
    fn send_requires_opponent(&self) -> bool {
        self.selected_kind == KIND_FRIEND && self.opponent.is_none()
    }

    /// Random sends are blocked until e11 (seek-based random opponent) —
    /// returns the localized "coming soon" status when kind=Random, else None.
    fn blocked_send_status(&self, loc: &str) -> Option<String> {
        if matches!(self.opponent_kind(), OpponentKind::Random) {
            Some(tr(loc, "play-kind-random-soon"))
        } else {
            None
        }
    }

    /// i18n key for the "sent" status; an empty opponent name means an open
    /// (Random) challenge.
    fn sent_status_key(opponent: &str) -> &'static str {
        if opponent.is_empty() {
            "play-status-open-sent"
        } else {
            "play-status-sent"
        }
    }

    /// The event signalling a successful send, per opponent kind. `game_id`
    /// is `Some` for Computer (AI games start immediately).
    pub(crate) fn send_event(kind: &OpponentKind, game_id: Option<String>) -> AppEvent {
        match kind {
            OpponentKind::Computer(_) => AppEvent::AiGameStarted {
                game_id: game_id.unwrap_or_default(),
                my_turn: true, // stream corrects the sidebar turn hint
            },
            OpponentKind::Friend(name) => AppEvent::ChallengeSent(name.clone()),
            OpponentKind::Random => AppEvent::ChallengeSent(String::new()),
        }
    }

    /// Spawn the challenge send, routed per `OpponentKind` + rated toggle.
    /// Result arrives as `ChallengeSent` / `AiGameStarted` /
    /// `ChallengeSendFailed` over the event channel.
    fn kick_send(&mut self, display: &Display) {
        let loc = i18n::locale_of(&self.config);
        if self.send_requires_opponent() {
            self.status = Some(tr(&loc, "play-status-select-first"));
            return;
        }
        // Random is disabled until e11 — never send an open challenge from here.
        if let Some(status) = self.blocked_send_status(&loc) {
            self.status = Some(status);
            return;
        }
        let Some(api) = self.app.online_idle_api() else {
            self.status = Some("offline backend".into());
            return;
        };
        if self.sending {
            return;
        }
        self.sending = true;
        let kind = self.opponent_kind();
        self.status = Some(match self.send_status_key() {
            "play-status-sending" => i18n::t(
                &loc,
                "play-status-sending",
                &[("name", kind.name().unwrap_or(""))],
            ),
            "play-status-sending-ai" => i18n::t(
                &loc,
                "play-status-sending-ai",
                &[("level", &kind.level().unwrap_or(0).to_string())],
            ),
            _ => tr(&loc, "play-status-sending-open"),
        });
        let rated = self.rated;
        let mode = self.mode();
        let tx = display.event_tx.clone();
        tokio::spawn(async move {
            let result: Result<AppEvent, String> = match &kind {
                OpponentKind::Friend(name) => api
                    .send_challenge(name, &mode, rated)
                    .await
                    .map(|_| Self::send_event(&kind, None))
                    .map_err(|e| e.to_string()),
                OpponentKind::Computer(level) => api
                    .send_challenge_ai(*level, &mode)
                    .await
                    .map(|g| Self::send_event(&kind, Some(g.id)))
                    .map_err(|e| e.to_string()),
                OpponentKind::Random => api
                    .create_open_challenge(&mode, rated)
                    .await
                    .map(|_| Self::send_event(&kind, None))
                    .map_err(|e| e.to_string()),
            };
            match result {
                Ok(ev) => {
                    let _ = tx.send(ev);
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::ChallengeSendFailed(e));
                }
            }
        });
    }

    /// What the shared selector row (row 2) hosts for the current state.
    /// Exactly one of the three is ever drawn (e06s07). When Computer AND
    /// Correspondence are both active, the last-selected one wins
    /// (`selector_focus`) instead of a fixed priority.
    fn selector_mode(&self) -> SelectorMode {
        let level_applicable = self.selected_kind == KIND_COMPUTER;
        let days_applicable = matches!(self.mode(), ChallengeMode::Correspondence(_));
        match (level_applicable, days_applicable) {
            (true, true) => match self.selector_focus {
                SelectorFocus::Level => SelectorMode::AiLevel,
                SelectorFocus::Days => SelectorMode::Days,
            },
            (true, false) => SelectorMode::AiLevel,
            (false, true) => SelectorMode::Days,
            (false, false) => SelectorMode::RandomInfo,
        }
    }

    /// Text for the selector row's fallback (`RandomInfo`) slot: the
    /// open-challenge hint when kind=Random, `None` (blank row) otherwise —
    /// for Friend there is nothing to show, the friend's name is on the
    /// Friend kind button (2026-11-11 adjustment).
    fn selector_info_text(&self, loc: &str) -> Option<String> {
        if self.selected_kind == KIND_RANDOM {
            Some(tr(loc, "play-random-info"))
        } else {
            None
        }
    }

    /// Hit-test the shared selector row at (x, y); returns true when a
    /// selector button consumed the touch.
    ///
    /// BUG-2026-11-11T153246: the level and days groups share the row, so only
    /// the group `selector_mode()` currently renders may be hit-tested —
    /// testing both unconditionally let the hidden one shadow the visible one
    /// (days-first order swallowed most level taps).
    fn selector_touch(&mut self, x: i16, y: i16) -> bool {
        match self.selector_mode() {
            SelectorMode::AiLevel => {
                for (i, btn) in self.level_buttons.iter().enumerate() {
                    if btn.rect.contains(x, y) {
                        self.select_level((i + 1) as u8);
                        return true;
                    }
                }
            }
            SelectorMode::Days => {
                // Days toggle group: one button per Lichess-allowed value.
                for (i, btn) in self.days_buttons.iter().enumerate() {
                    if btn.rect.contains(x, y) {
                        self.selected_days = ALLOWED_CORRESPONDENCE_DAYS[i];
                        self.selector_focus = SelectorFocus::Days;
                        if matches!(self.selected_mode, ChallengeMode::Correspondence(_)) {
                            self.selected_mode = ChallengeMode::Correspondence(self.selected_days);
                        }
                        return true;
                    }
                }
            }
            SelectorMode::RandomInfo => {}
        }
        false
    }

    /// The currently selected mode button, for drawing the selection marker.
    fn selected_mode_button(&self) -> &Button {
        match self.selected_mode {
            ChallengeMode::Unlimited => &self.mode_button_unlimited,
            ChallengeMode::RealTime => &self.mode_button_realtime,
            ChallengeMode::Correspondence(_) => &self.mode_button_correspondence,
        }
    }

    // ── e06s07: rated toggle + opponent kind + selector row ──────────────────

    /// The opponent kind matching the current selector state.
    pub fn opponent_kind(&self) -> OpponentKind {
        match self.selected_kind {
            KIND_COMPUTER => OpponentKind::Computer(self.ai_level),
            KIND_RANDOM => OpponentKind::Random,
            _ => OpponentKind::Friend(self.opponent.clone().unwrap_or_default()),
        }
    }

    pub fn select_kind(&mut self, idx: usize) {
        if idx < self.kind_buttons.len() {
            self.selected_kind = idx;
            // Selecting Computer makes the level selector the last-selected one.
            if idx == KIND_COMPUTER {
                self.selector_focus = SelectorFocus::Level;
            }
        }
    }

    /// Whether a kind button can be selected. Random is DISABLED until e11
    /// (seek-based random opponent via POST /api/board/seek) — the button is
    /// relabeled "coming soon" and taps are ignored.
    fn kind_selectable(idx: usize) -> bool {
        idx != KIND_RANDOM
    }

    /// Select a game mode; Correspondence makes the days selector the
    /// last-selected one on the shared row.
    pub fn select_mode(&mut self, mode: ChallengeMode) {
        if matches!(mode, ChallengeMode::Correspondence(_)) {
            self.selector_focus = SelectorFocus::Days;
        }
        self.selected_mode = mode;
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

    /// Apply a friend picked on the FriendlistScreen: remember the name and
    /// show it on the Friend kind button (with a clamped font for long names).
    pub fn apply_friend_selected(&mut self, name: String) {
        self.opponent = Some(name.clone());
        let btn = &mut self.kind_buttons[KIND_FRIEND];
        btn.label = name;
        btn.font_size = friend_label_font(btn.label.len());
    }

    /// e07s01: open the list-picker modal for friend selection. If friends
    /// are already cached, show the picker immediately; otherwise fetch
    /// them first (the picker appears when FriendsLoaded arrives).
    pub fn open_friend_picker(&mut self, display: &Display) {
        let loc = i18n::locale_of(&self.config);
        let title = tr(&loc, "friends-title");
        if let Some(friends) = &self.friends {
            let mut picker = crate::ui::widgets::ListPicker::new((**friends).clone(), title);
            picker.select_button.label = tr(&loc, "picker-select");
            picker.cancel_button.label = tr(&loc, "picker-cancel");
            picker.tap_to_select = true; // e07s08: tap a friend = immediate select
            self.list_picker = Some(picker);
        } else if !self.friends_loading {
            // Fetch friends; picker opens in the FriendsLoaded handler.
            if let Some(api) = self.app.online_idle_api() {
                self.friends_loading = true;
                let tx = display.event_tx.clone();
                tokio::spawn(async move {
                    match api.get_following().await {
                        Ok(names) => {
                            let _ = tx.send(AppEvent::FriendsLoaded(Arc::new(names)));
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::FriendsFailed(e.to_string()));
                        }
                    }
                });
            } else {
                // Review fix: offline — show status instead of silent no-op.
                let loc = i18n::locale_of(&self.config);
                self.status = Some(tr(&loc, "play-status-failed").replace("{error}", "offline"));
            }
        }
    }

    /// Rated toggle is hidden for Computer — Lichess AI games are always casual.
    /// When hidden the row stays blank so nothing below shifts.
    fn rated_visible(&self) -> bool {
        self.selected_kind != KIND_COMPUTER
    }

    fn selected_rated_button(&self) -> &Button {
        &self.rated_buttons[usize::from(self.rated)]
    }

    fn selected_kind_button(&self) -> &Button {
        &self.kind_buttons[self.selected_kind]
    }

    /// Rects of every button visible in the current state — used by the
    /// layout-invariant test to prove in-bounds + pairwise non-overlap.
    #[cfg(test)]
    fn visible_rects(&self) -> Vec<Rectangle> {
        let mut rects: Vec<Rectangle> = vec![
            self.mode_button_unlimited.rect,
            self.mode_button_realtime.rect,
            self.mode_button_correspondence.rect,
            self.send_button.rect,
            self.invites_button.rect,
            self.back_button.rect,
        ];
        rects.extend(self.kind_buttons.iter().map(|b| b.rect));
        if self.rated_visible() {
            rects.extend(self.rated_buttons.iter().map(|b| b.rect));
        }
        match self.selector_mode() {
            SelectorMode::AiLevel => rects.extend(self.level_buttons.iter().map(|b| b.rect)),
            SelectorMode::Days => rects.extend(self.days_buttons.iter().map(|b| b.rect)),
            SelectorMode::RandomInfo => {}
        }
        rects
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

        // Selection-only redraws skip the full-screen clear — buttons paint
        // their own white background, so overdraw is sufficient and the e-ink
        // panel doesn't flash. Full clear happens on first paint / push / expose.
        if self.skip_full_clear {
            self.skip_full_clear = false;
        } else {
            display.renderer.clear(DrawColor::White)?;
        }

        let loc = i18n::locale_of(&self.config);
        let title = tr(&loc, "play-title");
        layout::draw_title_bar(&mut display.renderer, &title)?;

        self.mode_button_unlimited.label = tr(&loc, "play-mode-unlimited");
        self.mode_button_realtime.label = tr(&loc, "play-mode-realtime");
        self.mode_button_correspondence.label = tr(&loc, "play-mode-correspondence");
        self.send_button.label = tr(&loc, "play-send");
        self.rated_buttons[0].label = tr(&loc, "play-casual");
        self.rated_buttons[1].label = tr(&loc, "play-rated");
        self.kind_buttons[KIND_COMPUTER].label = tr(&loc, "play-kind-computer");
        // Random disabled until e11 — relabeled "coming soon".
        self.kind_buttons[KIND_RANDOM].label = tr(&loc, "play-kind-random-soon");
        // The Friend button carries the picked friend's name (font clamped for
        // long names); falls back to the localized "Friend" label.
        let (friend_label, friend_font) = match &self.opponent {
            Some(name) => (name.clone(), friend_label_font(name.len())),
            None => (tr(&loc, "play-kind-friend"), 32.0),
        };
        self.kind_buttons[KIND_FRIEND].label = friend_label;
        self.kind_buttons[KIND_FRIEND].font_size = friend_font;

        self.invites_button.draw(&mut display.renderer)?;

        // Row 1: kind group (Computer | Friend | Random).
        for btn in &self.kind_buttons {
            btn.draw(&mut display.renderer)?;
        }
        draw_selection_marker(&mut display.renderer, &self.selected_kind_button().rect)?;

        // Row 2: shared selector — exactly one of level row / days row / info.
        match self.selector_mode() {
            SelectorMode::AiLevel => {
                // "AI level" label centered above the row (same horizontal
                // centering as the days label below), with the SAME margin to
                // the selector row as the days label has (SELECTOR_LABEL_MARGIN
                // below it) — clears the kind row above (2026-11-11 fix).
                let level_label = tr(&loc, "play-ai-level");
                let (lw, lh) = display.renderer.measure_text(&level_label, 24.0);
                display.renderer.draw_text(
                    (layout::SCREEN_W - lw as i16) / 2,
                    SELECTOR_Y - SELECTOR_LABEL_MARGIN - lh as i16,
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
            SelectorMode::Days => {
                for btn in &self.days_buttons {
                    btn.draw(&mut display.renderer)?;
                }
                draw_selection_marker(
                    &mut display.renderer,
                    &self.days_buttons[self.selected_days_index()].rect,
                )?;
                // "Days per move" label BELOW the days selector row, same
                // margin from the row as the AI-level label above.
                let days_label = tr(&loc, "play-days-label");
                let (dw, _) = display.renderer.measure_text(&days_label, 24.0);
                display.renderer.draw_text(
                    (layout::SCREEN_W - dw as i16) / 2,
                    SELECTOR_Y + SELECTOR_H as i16 + SELECTOR_LABEL_MARGIN,
                    &days_label,
                    24.0,
                    DrawColor::Black,
                )?;
            }
            SelectorMode::RandomInfo => {
                // Fallback slot: the open-challenge hint for Random; the row
                // stays BLANK for Friend (2026-11-11 adjustment) — the picked
                // friend's name already lives on the Friend kind button.
                if let Some(info) = self.selector_info_text(&loc) {
                    let (iw, ih) = display.renderer.measure_text(&info, 24.0);
                    display.renderer.draw_text(
                        (layout::SCREEN_W - iw as i16) / 2,
                        SELECTOR_Y + (SELECTOR_H as i16 - ih as i16) / 2,
                        &info,
                        24.0,
                        DrawColor::Black,
                    )?;
                }
            }
        }

        // Row 3: mode group.
        self.mode_button_unlimited.draw(&mut display.renderer)?;
        self.mode_button_realtime.draw(&mut display.renderer)?;
        self.mode_button_correspondence
            .draw(&mut display.renderer)?;
        // Selection marker on the active mode button (selected-file style).
        draw_selection_marker(&mut display.renderer, &self.selected_mode_button().rect)?;

        // Row 4: rated toggle (hidden for Computer; the row stays blank).
        if self.rated_visible() {
            for btn in &self.rated_buttons {
                btn.draw(&mut display.renderer)?;
            }
            draw_selection_marker(&mut display.renderer, &self.selected_rated_button().rect)?;
        }

        // Lower half: Send centered.
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

        // e07s01: draw the list-picker modal on top of everything.
        if let Some(picker) = &self.list_picker {
            picker.draw(&mut display.renderer)?;
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
            // Bubbled up from the popped FriendlistScreen: the picked name
            // replaces the Friend kind button's label (e06s07).
            AppEvent::FriendSelected(name) => {
                info!("Opponent selected: {}", name);
                self.apply_friend_selected(name);
                self.status = None;
                self.skip_full_clear = true;
                Ok(Transition::Redraw)
            }

            // e07s01: friends list loaded — open the picker modal.
            AppEvent::FriendsLoaded(names) => {
                self.friends_loading = false;
                let loc = i18n::locale_of(&self.config);
                let title = tr(&loc, "friends-title");
                let mut picker = crate::ui::widgets::ListPicker::new((*names).clone(), title);
                picker.select_button.label = tr(&loc, "picker-select");
                picker.cancel_button.label = tr(&loc, "picker-cancel");
                picker.tap_to_select = true; // e07s08: tap a friend = immediate select
                self.friends = Some(names.clone());
                self.list_picker = Some(picker);
                Ok(Transition::Redraw)
            }

            AppEvent::FriendsFailed(err) => {
                self.friends_loading = false;
                warn!("Friends fetch failed: {err}");
                let loc = i18n::locale_of(&self.config);
                self.status = Some(i18n::t(&loc, "common-error", &[("error", &err)]));
                Ok(Transition::Redraw)
            }

            AppEvent::ChallengeSent(opponent) => {
                self.sending = false;
                let loc = i18n::locale_of(&self.config);
                self.status = Some(match Self::sent_status_key(&opponent) {
                    "play-status-open-sent" => tr(&loc, "play-status-open-sent"),
                    _ => i18n::t(&loc, "play-status-sent", &[("name", &opponent)]),
                });
                // Show the pending outgoing invite on the Invites screen.
                Ok(Transition::Push(Box::new(InvitesScreen::new(
                    self.app.clone(),
                    self.config.clone(),
                ))))
            }

            AppEvent::AiGameStarted { game_id, my_turn } => {
                self.sending = false;
                info!("AI game started immediately: {game_id}");
                let game_app = self.app.clone().attach_game(game_id, my_turn);
                let mut game_screen = ChessGameScreen::new(game_app, self.config.clone());
                // e06s04: carry the challenge settings so a rematch can
                // re-issue the same AI challenge after game over.
                game_screen.set_rematch(RematchContext {
                    kind: self.opponent_kind(),
                    mode: self.mode(),
                    rated: self.rated,
                });
                Ok(Transition::Push(Box::new(game_screen)))
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

                // e07s01: if the list-picker modal is open, it captures all input.
                if let Some(picker) = &mut self.list_picker {
                    use crate::ui::widgets::PickResult;
                    match picker.handle_touch(touch.x, touch.y) {
                        PickResult::Selected(idx) => {
                            if let Some(name) = picker.items.get(idx).cloned() {
                                self.apply_friend_selected(name);
                            }
                            self.list_picker = None;
                            self.skip_full_clear = true;
                            return Ok(Transition::Redraw);
                        }
                        PickResult::Cancelled => {
                            self.list_picker = None;
                            return Ok(Transition::Redraw);
                        }
                        PickResult::NotHandled => {
                            // Item tapped (selection updated) — redraw to show highlight.
                            return Ok(Transition::Redraw);
                        }
                    }
                }

                if self.back_button.rect.contains(touch.x, touch.y) {
                    return Ok(Transition::Pop);
                }

                if self.mode_button_unlimited.rect.contains(touch.x, touch.y) {
                    self.select_mode(ChallengeMode::Unlimited);
                    self.skip_full_clear = true;
                    return Ok(Transition::Redraw);
                }
                if self.mode_button_realtime.rect.contains(touch.x, touch.y) {
                    self.select_mode(ChallengeMode::RealTime);
                    self.skip_full_clear = true;
                    return Ok(Transition::Redraw);
                }
                if self
                    .mode_button_correspondence
                    .rect
                    .contains(touch.x, touch.y)
                {
                    let days = self.selected_days;
                    self.select_mode(ChallengeMode::Correspondence(days));
                    self.skip_full_clear = true;
                    return Ok(Transition::Redraw);
                }

                // Shared selector row (level | days | info) — see selector_touch.
                if self.selector_touch(touch.x, touch.y) {
                    self.skip_full_clear = true;
                    return Ok(Transition::Redraw);
                }

                // e06: rated toggle + kind selector.
                for (i, btn) in self.rated_buttons.iter().enumerate() {
                    if btn.rect.contains(touch.x, touch.y) {
                        self.select_rated(i == 1);
                        self.skip_full_clear = true;
                        return Ok(Transition::Redraw);
                    }
                }
                for (i, btn) in self.kind_buttons.iter().enumerate() {
                    if btn.rect.contains(touch.x, touch.y) {
                        // Random is disabled until e11 — tap is ignored.
                        if !Self::kind_selectable(i) {
                            return Ok(Transition::Stay);
                        }
                        self.select_kind(i);
                        // e07s01: tapping the Friend kind button selects
                        // kind=Friend AND opens the ListPicker modal (replaces
                        // the old FriendlistScreen push); the picked name
                        // lands via apply_friend_selected.
                        if i == KIND_FRIEND {
                            info!("Opening friend picker modal");
                            self.open_friend_picker(display);
                            return Ok(Transition::Redraw);
                        }
                        self.skip_full_clear = true;
                        return Ok(Transition::Redraw);
                    }
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
    fn friend_button_label_reflects_selection() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        assert_eq!(s.friend_button_label().0, "Friend");
        s.apply_friend_selected("Alice42".into());
        assert_eq!(s.kind_buttons[KIND_FRIEND].label, "Alice42");
        assert_eq!(s.friend_button_label().0, "Alice42");
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

    // ── e06s07: shared selector row ──────────────────────────────────────────

    #[test]
    fn selector_row_hosts_exactly_one_of_level_days_info() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        // Computer -> AI level row.
        s.select_kind(KIND_COMPUTER);
        s.select_mode(ChallengeMode::RealTime);
        assert_eq!(s.selector_mode(), SelectorMode::AiLevel);
        // Friend + Correspondence -> days row.
        s.select_kind(KIND_FRIEND);
        s.select_mode(ChallengeMode::Correspondence(3));
        assert_eq!(s.selector_mode(), SelectorMode::Days);
        // Friend + non-correspondence -> info text.
        s.select_mode(ChallengeMode::RealTime);
        assert_eq!(s.selector_mode(), SelectorMode::RandomInfo);
        s.select_mode(ChallengeMode::Unlimited);
        assert_eq!(s.selector_mode(), SelectorMode::RandomInfo);
        // Random + non-correspondence -> info text; Random + Correspondence -> days.
        s.select_kind(KIND_RANDOM);
        assert_eq!(s.selector_mode(), SelectorMode::RandomInfo);
        s.select_mode(ChallengeMode::Correspondence(5));
        assert_eq!(s.selector_mode(), SelectorMode::Days);
    }

    // 2026-11-11 adjustment: the selector row's fallback slot is blank when
    // Friend is picked (the friend's name lives on the Friend kind button);
    // only Random shows the open-challenge info text.
    #[test]
    fn selector_fallback_blank_for_friend_info_for_random() {
        let s = PlayScreen::new(ChessApp::new_offline(), test_config());
        let loc = i18n::locale_of(&s.config);
        let mut s = s;
        s.select_kind(KIND_FRIEND);
        s.select_mode(ChallengeMode::RealTime);
        assert_eq!(s.selector_mode(), SelectorMode::RandomInfo);
        assert_eq!(
            s.selector_info_text(&loc),
            None,
            "Friend must leave the selector row blank"
        );
        s.select_kind(KIND_RANDOM);
        assert_eq!(s.selector_mode(), SelectorMode::RandomInfo);
        let info = s.selector_info_text(&loc).expect("Random shows info text");
        assert!(!info.is_empty());
    }

    // e06 close-out: Random is disabled + relabeled "coming soon" until e11
    // (seek-based random opponent) — taps are ignored, sends are blocked.
    #[test]
    fn random_kind_disabled_until_e11() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        let loc = i18n::locale_of(&s.config);
        assert!(!PlayScreen::kind_selectable(KIND_RANDOM));
        assert!(PlayScreen::kind_selectable(KIND_COMPUTER));
        assert!(PlayScreen::kind_selectable(KIND_FRIEND));
        // Programmatic select still maps, but sending is blocked with the
        // "coming soon" status instead of create_open_challenge.
        s.select_kind(KIND_RANDOM);
        assert_eq!(s.opponent_kind(), OpponentKind::Random);
        let blocked = s
            .blocked_send_status(&loc)
            .expect("Random send must be blocked");
        assert_eq!(blocked, tr(&loc, "play-kind-random-soon"));
        s.select_kind(KIND_FRIEND);
        assert_eq!(s.blocked_send_status(&loc), None);
    }

    // User design 2026-11-11: Computer + Correspondence both active -> the
    // LAST selected one is rendered on the shared selector row.
    #[test]
    fn computer_and_correspondence_show_last_selected_selector() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        s.select_kind(KIND_COMPUTER);
        s.select_mode(ChallengeMode::Correspondence(3));
        // Correspondence was selected last -> days row.
        assert_eq!(s.selector_mode(), SelectorMode::Days);
        // Tap Computer again -> level row.
        s.select_kind(KIND_COMPUTER);
        assert_eq!(s.selector_mode(), SelectorMode::AiLevel);
        // Tap Correspondence again -> days row.
        s.select_mode(ChallengeMode::Correspondence(7));
        assert_eq!(s.selector_mode(), SelectorMode::Days);
        // Mode leaving Correspondence -> level row (Computer still active).
        s.select_mode(ChallengeMode::RealTime);
        assert_eq!(s.selector_mode(), SelectorMode::AiLevel);
        // Kind leaving Computer -> days gone with it -> info.
        s.select_mode(ChallengeMode::Correspondence(2));
        s.select_kind(KIND_FRIEND);
        assert_eq!(s.selector_mode(), SelectorMode::Days);
        s.select_mode(ChallengeMode::Unlimited);
        assert_eq!(s.selector_mode(), SelectorMode::RandomInfo);
    }

    // BUG-2026-11-11T153246: in Computer mode, ONE tap on any level button
    // must select that level — the hidden days group must not shadow them.
    #[test]
    fn every_level_button_selectable_with_one_tap_in_computer_mode() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        s.select_kind(KIND_COMPUTER);
        assert_eq!(s.selector_mode(), SelectorMode::AiLevel);
        let rects: Vec<Rectangle> = s.level_buttons.iter().map(|b| b.rect).collect();
        for (i, r) in rects.iter().enumerate() {
            let cx = r.x + r.width as i16 / 2;
            let cy = r.y + r.height as i16 / 2;
            assert!(
                s.selector_touch(cx, cy),
                "tap on level button {} was not consumed",
                i + 1
            );
            assert_eq!(
                s.ai_level,
                (i + 1) as u8,
                "tap on level {} must select level {}",
                i + 1,
                i + 1
            );
        }
    }

    #[test]
    fn every_days_button_selectable_in_correspondence_mode() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        s.select_kind(KIND_FRIEND);
        s.selected_mode = ChallengeMode::Correspondence(3);
        assert_eq!(s.selector_mode(), SelectorMode::Days);
        let rects: Vec<Rectangle> = s.days_buttons.iter().map(|b| b.rect).collect();
        for (&day, r) in ALLOWED_CORRESPONDENCE_DAYS.iter().zip(&rects) {
            let cx = r.x + r.width as i16 / 2;
            let cy = r.y + r.height as i16 / 2;
            assert!(s.selector_touch(cx, cy));
            assert_eq!(s.selected_days, day);
            assert_eq!(s.mode(), ChallengeMode::Correspondence(day));
        }
    }

    #[test]
    fn friend_button_font_clamps_for_long_names() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        s.apply_friend_selected("Alice".into());
        assert_eq!(s.kind_buttons[KIND_FRIEND].font_size, 32.0);
        s.apply_friend_selected("BartholomewLong".into()); // 15 chars
        assert_eq!(s.kind_buttons[KIND_FRIEND].font_size, 26.0);
        s.apply_friend_selected("AVeryLongLichessUsername".into()); // 24 chars
        assert_eq!(s.kind_buttons[KIND_FRIEND].font_size, 20.0);
        assert_eq!(
            s.kind_buttons[KIND_FRIEND].label,
            "AVeryLongLichessUsername"
        );
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

    // ── e06s07: layout invariants (user-approved design) ────────────────────

    #[test]
    fn e06s07_rows_1_to_4_end_above_screen_half_and_send_is_lower_half_centered() {
        let s = PlayScreen::new(ChessApp::new_offline(), test_config());
        let half = layout::SCREEN_H / 2; // 724
        // Row 1: kind group.
        for b in &s.kind_buttons {
            assert!(
                b.rect.y + b.rect.height as i16 <= half,
                "kind row below half: {:?}",
                b.rect
            );
        }
        // Row 2: selector band (level and days share the row).
        for b in s.level_buttons.iter().chain(s.days_buttons.iter()) {
            assert_eq!(b.rect.y, SELECTOR_Y);
            assert!(
                b.rect.y + b.rect.height as i16 <= half,
                "selector row below half"
            );
        }
        // Row 3: mode group.
        for b in [
            &s.mode_button_unlimited,
            &s.mode_button_realtime,
            &s.mode_button_correspondence,
        ] {
            assert!(
                b.rect.y + b.rect.height as i16 <= half,
                "mode row below half"
            );
        }
        // Row 4: rated toggle — last row of the upper half.
        for b in &s.rated_buttons {
            assert!(
                b.rect.y + b.rect.height as i16 <= half,
                "rated row below half"
            );
        }
        // Send centered in the lower half: center == (SCREEN_W/2, (half + SCREEN_H)/2).
        let send = &s.send_button.rect;
        let cx = send.x + send.width as i16 / 2;
        let cy = send.y + send.height as i16 / 2;
        assert_eq!(cx, layout::SCREEN_W / 2, "send not horizontally centered");
        assert_eq!(
            cy,
            (half + layout::SCREEN_H) / 2,
            "send not lower-half centered"
        );
        assert!(send.y >= half, "send must live in the lower half");
    }

    // 2026-11-11 fix: the AI-level label touched the Friend button because the
    // selector row left only a 30px gap above it (exactly the 24px text height,
    // 30px measured: ascent 24 + descent 6). The row now sits with equal
    // margins above and below, leaving room for the label + its margin.
    #[test]
    fn e06s07_selector_row_has_equal_margins_and_label_clears_kind_row() {
        let s = PlayScreen::new(ChessApp::new_offline(), test_config());
        let kind_bottom = KIND_Y + KIND_H as i16;
        let selector_bottom = SELECTOR_Y + SELECTOR_H as i16;
        let gap_above = SELECTOR_Y - kind_bottom;
        let gap_below = MODE_Y - selector_bottom;
        assert_eq!(
            gap_above, gap_below,
            "margin above the selector row must equal the margin below it"
        );
        // 24px label text is 30px tall (ascent 24 + descent 6, measured
        // 2026-11-11); with its SELECTOR_LABEL_MARGIN to the row it must still
        // clear the kind row — no overlap with the Friend button.
        const TEXT_H_24PX: i16 = 30;
        assert!(
            gap_above >= TEXT_H_24PX + SELECTOR_LABEL_MARGIN,
            "AI-level label (30px + {SELECTOR_LABEL_MARGIN}px margin) would overlap the kind row: gap={gap_above}"
        );
        assert_eq!(s.level_buttons[0].rect.y, SELECTOR_Y);
        assert_eq!(s.days_buttons[0].rect.y, SELECTOR_Y);
    }

    fn assert_visible_layout_ok(s: &PlayScreen, ctx: &str) {
        let rects = s.visible_rects();
        for r in &rects {
            assert!(r.x >= 0 && r.y >= 0, "{ctx}: negative origin: {r:?}");
            assert!(
                r.x + r.width as i16 <= layout::SCREEN_W,
                "{ctx}: overflows right: {r:?}"
            );
            assert!(
                r.y + r.height as i16 <= layout::SCREEN_H,
                "{ctx}: overflows bottom: {r:?}"
            );
        }
        for (i, a) in rects.iter().enumerate() {
            for b in rects.iter().skip(i + 1) {
                assert!(!rects_overlap(a, b), "{ctx}: overlap: {a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn e06s07_visible_buttons_in_bounds_and_non_overlapping_in_every_state() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        let modes = [
            ChallengeMode::Unlimited,
            ChallengeMode::RealTime,
            ChallengeMode::Correspondence(3),
        ];
        for kind in [KIND_COMPUTER, KIND_FRIEND, KIND_RANDOM] {
            for mode in modes {
                s.select_kind(kind);
                s.select_mode(mode);
                assert_visible_layout_ok(&s, &format!("kind={kind} mode={mode:?}"));
            }
        }
        // Both selectors applicable: each focus variant must also be clean.
        s.select_kind(KIND_COMPUTER);
        s.select_mode(ChallengeMode::Correspondence(3));
        s.selector_focus = SelectorFocus::Level;
        assert_visible_layout_ok(&s, "computer+correspondence, focus=level");
        s.selector_focus = SelectorFocus::Days;
        assert_visible_layout_ok(&s, "computer+correspondence, focus=days");
    }

    #[test]
    fn e06_defaults_are_casual_friend_realtime_level_3() {
        let s = PlayScreen::new(ChessApp::new_offline(), test_config());
        assert!(!s.rated);
        assert_eq!(s.selected_kind, KIND_FRIEND);
        assert_eq!(s.ai_level, 3);
        assert_eq!(s.opponent_kind(), OpponentKind::Friend(String::new()));
    }

    #[test]
    fn e06_opponent_kind_mapping() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        s.opponent = Some("Alice".into());
        s.select_kind(KIND_FRIEND);
        assert_eq!(s.opponent_kind(), OpponentKind::Friend("Alice".into()));
        s.select_kind(KIND_COMPUTER);
        s.select_level(7);
        assert_eq!(s.opponent_kind(), OpponentKind::Computer(7));
        s.select_kind(KIND_RANDOM);
        assert_eq!(s.opponent_kind(), OpponentKind::Random);
    }

    #[test]
    fn e06_rated_toggle_and_visibility_rules() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        assert!(s.rated_visible()); // Friend: rated toggle shown
        s.select_rated(true);
        assert!(s.rated);
        s.select_kind(KIND_COMPUTER); // Computer: AI is always casual -> toggle hidden
        assert!(!s.rated_visible());
        assert_eq!(s.selector_mode(), SelectorMode::AiLevel);
        s.select_kind(KIND_RANDOM); // Random: open challenge can be rated
        assert!(s.rated_visible());
        assert_ne!(s.selector_mode(), SelectorMode::AiLevel);
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
        s.select_kind(KIND_FRIEND);
        assert_eq!(s.send_status_key(), "play-status-sending");
        s.select_kind(KIND_COMPUTER);
        assert_eq!(s.send_status_key(), "play-status-sending-ai");
        s.select_kind(KIND_RANDOM);
        assert_eq!(s.send_status_key(), "play-status-sending-open");
    }

    #[test]
    fn e06s03_friend_without_opponent_needs_selection_first() {
        let mut s = PlayScreen::new(ChessApp::new_offline(), test_config());
        s.select_kind(KIND_FRIEND);
        s.opponent = None;
        assert!(s.send_requires_opponent());
        s.opponent = Some("Alice".into());
        assert!(!s.send_requires_opponent());
        s.select_kind(KIND_COMPUTER); // Computer: no opponent name needed
        assert!(!s.send_requires_opponent());
        s.select_kind(KIND_RANDOM); // Random: open challenge, no opponent name needed
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
