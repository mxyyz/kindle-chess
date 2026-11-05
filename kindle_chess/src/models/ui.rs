use std::{
    error::Error,
    sync::{
        Arc, RwLock,
        mpsc::{Receiver, Sender},
    },
    time::Instant,
};

use image::{ImageBuffer, Luma};

use crate::{
    api::github::UpdateInfo,
    models::{
        board_api::{ALLOWED_CORRESPONDENCE_DAYS, ChallengeMode, ChallengeSummary, GameDataList},
        chess::ChessApp,
        oauth::TokenInfo,
        puzzle::{PuzzleParams, PuzzleSession},
    },
    ui::{
        events::{AppEvent, Rectangle, RectangleExt},
        renderer::Renderer,
        widgets::{
            BoardWidget, Button, IconButton, PuzzleSidebarWidget, SidebarWidget,
            icon_button::{ICON_BUTTON_SIZE, Icon, load_icon},
        },
    },
    utils::config::AppConfig,
};

// ─── Display ──────────────────────────────────────────────────────────────────
// The single long-lived X11 resource. Created once in main() and borrowed by
// every Screen implementation for drawing and event routing.

pub struct Display {
    pub renderer: Renderer,
    pub conn: Arc<x11rb::rust_connection::RustConnection>,
    pub event_tx: Sender<AppEvent>,
    pub event_rx: Receiver<AppEvent>,

    // Triple-tap detection lives here because it is global (works on any screen)
    pub tap_times: Vec<Instant>,
    pub last_tap_pos: Option<(i16, i16)>,
}

// ─── Screen ───────────────────────────────────────────────────────────────────
// A Screen owns only widgets and screen-local state.
// It borrows Display for drawing and returns a Transition to drive navigation.

pub trait Screen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn Error>>;
    fn handle_event(
        &mut self,
        event: AppEvent,
        display: &mut Display,
    ) -> Result<Transition, Box<dyn Error>>;

    /// Called when this screen becomes the top of the stack again because a
    /// screen above it was popped. While it was covered, the framebuffer was
    /// drawn over by the screen on top — so any widget that does partial/diff
    /// rendering (the board) must be invalidated here, and screens showing
    /// fetched data (ongoing games) can refresh. Default: no-op.
    fn on_reveal(&mut self) {}
}

// ─── Transition ───────────────────────────────────────────────────────────────

pub enum Transition {
    Stay,                  // keep current screen, no redraw needed
    Redraw,                // keep current screen, request a redraw
    Push(Box<dyn Screen>), // navigate forward to a new screen
    Pop,                   // return to the previous screen
    Quit,                  // exit the application
}

// ─── HomeScreen ───────────────────────────────────────────────────────────────
// The top-level launcher. Add a button here for every future game.

pub struct HomeScreen {
    pub chess_button: Button,
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
        // the four equal buttons are stacked and height-centred in the lower
        // half.
        const BTN_W: u16 = 600;
        const BTN_H: u16 = 96;
        const GAP: i16 = 28;
        const CENTER_X: i16 = 1072 / 2;
        let x = CENTER_X - BTN_W as i16 / 2;
        let total = 4 * BTN_H as i16 + 3 * GAP;
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

// ─── ChessGameScreen ──────────────────────────────────────────────────────────

pub struct ChessGameScreen {
    pub app: ChessApp,
    pub board: BoardWidget,
    pub sidebar: SidebarWidget,
    // First-render guard: kick the game-state stream task exactly once.
    pub stream_started: bool,
    // Cancel flag of the currently running stream task. Set on DeviceWake so
    // a stalled stream (device was locked) is superseded by a fresh one.
    pub stream_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl ChessGameScreen {
    pub fn new(app: ChessApp) -> Self {
        // Seed the sidebar with the initial turn from `attach_game` so the
        // sidebar shows "Your turn" / "Waiting…" immediately, before the
        // game-state stream catches up.
        let mut sidebar = SidebarWidget::new(Rectangle::new(0, 1072, 1072, 376));
        if let Some(turn) = app.turn() {
            sidebar.set_turn(turn.clone());
        }
        Self {
            app,
            board: BoardWidget::new(Rectangle::new(0, 0, 1072, 1072)),
            sidebar,
            stream_started: false,
            stream_cancel: None,
        }
    }
}

// ─── ChessGameScreen ──────────────────────────────────────────────────────────

pub struct OngoingChessGamesScreen {
    pub app: ChessApp,
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
    pub fn new(app: ChessApp) -> Self {
        // Layout: 1072 × 1448 total canvas
        const BTN_W: u16 = 800;
        const BTN_H: u16 = 120;
        const CENTER_X: i16 = 1072 / 2; // 336
        const CENTER_Y: i16 = 1448 / 2; // 304
        Self {
            app,
            prev_page_button: IconButton::new(
                CENTER_X - (ICON_BUTTON_SIZE as i16 + 8),
                CENTER_Y * 2 - (ICON_BUTTON_SIZE as i16 - 20) - 32,
                "back-page.png",
            ),
            next_page_button: IconButton::new(
                CENTER_X + 8,
                CENTER_Y * 2 - (ICON_BUTTON_SIZE as i16 - 20) - 32,
                "next-page.png",
            ),
            chessgame_button_0: Button::new(
                CENTER_X - (8 + BTN_W as i16 / 2),
                CENTER_Y - (BTN_H as i16 / 2 + 10),
                BTN_W / 2,
                BTN_H,
                "-".to_string(),
                40.0,
                true,
            ),
            chessgame_button_1: Button::new(
                CENTER_X + 8,
                CENTER_Y - (BTN_H as i16 / 2 + 10),
                BTN_W / 2,
                BTN_H,
                "-".to_string(),
                40.0,
                true,
            ),
            chessgame_button_2: Button::new(
                CENTER_X - (8 + BTN_W as i16 / 2),
                CENTER_Y + (BTN_H as i16 / 2 + 10),
                BTN_W / 2,
                BTN_H,
                "-".to_string(),
                40.0,
                true,
            ),
            chessgame_button_3: Button::new(
                CENTER_X + 8,
                CENTER_Y + (BTN_H as i16 / 2 + 10),
                BTN_W / 2,
                BTN_H,
                String::from("-"),
                40.0,
                true,
            ),
            back_button: IconButton::new(
                32 + ICON_BUTTON_SIZE as i16 / 2,
                CENTER_Y * 2 - (ICON_BUTTON_SIZE as i16 - 20) - 32,
                "back-nav.png",
            ),

            games: None,
            error: None,
            loading: false,
            page_index: 0,
        }
    }
}

// ─── InvitesScreen ────────────────────────────────────────────────────────────
/// Incoming/outgoing friend challenges. Polls the challenge list every 5 s;
/// incoming rows get Accept/Decline, outgoing rows get Cancel. Any challenge
/// whose status flips to "accepted" starts the live game (push ChessGameScreen).
pub struct InvitesScreen {
    pub app: ChessApp,
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
    pub fn new(app: ChessApp) -> Self {
        // Layout: 1072 × 1448. "Incoming" section at top (4 rows), "Outgoing"
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
        let bottom = 1448 - (ICON_BUTTON_SIZE as i16 - 20) - 32;
        // Section label rows: incoming label y=200 (icons y=176), outgoing
        // label y=680 (icons y=656) — icons at the right edge of the canvas.
        let icon_next_x = 1072 - ICON_BUTTON_SIZE as i16 - 24;
        let icon_prev_x = icon_next_x - ICON_BUTTON_SIZE as i16 - 16;
        Self {
            app,
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

// ─── PlayScreen ───────────────────────────────────────────────────────────────
/// The Play flow's form: pick a game mode (Unlimited / Real time 10:5 /
/// Correspondence with a 1–14 days spinner), pick an opponent via
/// FriendlistScreen, then send a casual standard challenge with random color.
pub struct PlayScreen {
    pub app: ChessApp,
    pub mode_button_unlimited: Button,
    pub mode_button_realtime: Button,
    pub mode_button_correspondence: Button,
    /// Days-per-move toggle group — only drawn in correspondence mode. One
    /// button per Lichess-allowed value (ALLOWED_CORRESPONDENCE_DAYS); a
    /// button group instead of a slider/spinner so rapid taps can't hit the
    /// triple-tap-to-quit gesture and values can't be invalid.
    pub days_buttons: [Button; 7],
    pub selected_days: u8,
    pub opponent_button: Button,
    pub send_button: Button,
    pub invites_button: IconButton,
    pub back_button: IconButton,

    pub selected_mode: ChallengeMode,
    pub opponent: Option<String>,
    pub status: Option<String>,
    pub sending: bool,
}

impl PlayScreen {
    pub fn new(app: ChessApp) -> Self {
        // Layout: 1072 × 1448. Title top, invites icon top-right, mode row,
        // spinner row, opponent + send buttons, status line, back bottom-left.
        const MODE_W: u16 = 320;
        const MODE_H: u16 = 80;
        const BTN_W: u16 = 600;
        const BTN_H: u16 = 96;
        let mode_x0 = (1072 - (3 * MODE_W as i16 + 2 * 16)) / 2;
        let x = (1072 - BTN_W as i16) / 2;
        // Days toggle group: 7 buttons of 100px + 6 gaps of 16 = 796px wide,
        // centred, on the slider's old row (y=310).
        const DAY_W: u16 = 100;
        let days_x0 = (1072 - (7 * DAY_W as i16 + 6 * 16)) / 2;
        let days_labels = ["1", "2", "3", "5", "7", "10", "14"];
        let bottom = 1448 - (ICON_BUTTON_SIZE as i16 - 20) - 32;
        Self {
            app,
            mode_button_unlimited: Button::new(mode_x0, 200, MODE_W, MODE_H, "Unlimited".into(), 32.0, true),
            mode_button_realtime: Button::new(mode_x0 + MODE_W as i16 + 16, 200, MODE_W, MODE_H, "Real time 10:5".into(), 32.0, true),
            mode_button_correspondence: Button::new(mode_x0 + 2 * (MODE_W as i16 + 16), 200, MODE_W, MODE_H, "Correspondence".into(), 32.0, true),
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
            opponent_button: Button::new(x, 470, BTN_W, BTN_H, "Opponent: -".into(), 40.0, true),
            send_button: Button::new(x, 600, BTN_W, BTN_H, "Send challenge".into(), 40.0, true),
            invites_button: IconButton::new(1072 - ICON_BUTTON_SIZE as i16 - 24, 24, "invitation.png"),
            back_button: IconButton::new(32 + ICON_BUTTON_SIZE as i16 / 2, bottom, "back-nav.png"),
            selected_mode: ChallengeMode::RealTime,
            opponent: None,
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

// ─── FriendlistScreen ─────────────────────────────────────────────────────────
/// Opponent picker for the Play flow: paged list of the user's Lichess friends
/// (`/api/rel/following`). Tapping a friend emits `AppEvent::FriendSelected`
/// and pops, handing the username back to the pushing screen (PlayScreen).
pub struct FriendlistScreen {
    pub app: ChessApp,
    pub friend_button_0: Button,
    pub friend_button_1: Button,
    pub friend_button_2: Button,
    pub friend_button_3: Button,
    pub prev_page_button: IconButton,
    pub next_page_button: IconButton,
    pub back_button: IconButton,

    // Async fetch state. `friends == None && error == None && !loading` means
    // the screen has not yet kicked off its initial fetch — `render` triggers it.
    pub friends: Option<Arc<Vec<String>>>,
    pub error: Option<String>,
    pub loading: bool,

    // Pagination: 4 friend buttons per page; labels baked by `set_page`.
    pub page_index: usize,
}

impl FriendlistScreen {
    pub fn new(app: ChessApp) -> Self {
        // Layout: 1072 × 1448. Title strip at top, then 4 stacked 600×96
        // buttons starting at y=400, page icons + back along the bottom.
        const BTN_W: u16 = 600;
        const BTN_H: u16 = 96;
        const GAP: i16 = 28;
        const START_Y: i16 = 400;
        let x = (1072 - BTN_W as i16) / 2;
        let row = |i: i16| START_Y + i * (BTN_H as i16 + GAP);
        let bottom = 1448 - (ICON_BUTTON_SIZE as i16 - 20) - 32;
        Self {
            app,
            friend_button_0: Button::new(x, row(0), BTN_W, BTN_H, "-".to_string(), 40.0, true),
            friend_button_1: Button::new(x, row(1), BTN_W, BTN_H, "-".to_string(), 40.0, true),
            friend_button_2: Button::new(x, row(2), BTN_W, BTN_H, "-".to_string(), 40.0, true),
            friend_button_3: Button::new(x, row(3), BTN_W, BTN_H, "-".to_string(), 40.0, true),
            prev_page_button: IconButton::new(1072 / 2 - (ICON_BUTTON_SIZE as i16 + 8), bottom, "back-page.png"),
            next_page_button: IconButton::new(1072 / 2 + 8, bottom, "next-page.png"),
            back_button: IconButton::new(32 + ICON_BUTTON_SIZE as i16 / 2, bottom, "back-nav.png"),
            friends: None,
            error: None,
            loading: false,
            page_index: 0,
        }
    }
}

// ─── ChessAuthScreen ──────────────────────────────────────────────────────────

pub struct ChessAuthScreen {
    pub qr_code: Rectangle,
    pub auth_status: Rectangle,
    pub qr_image: Option<ImageBuffer<Luma<u8>, Vec<u8>>>,
    pub auth_url: Option<String>,
    // First-render flag: kick the QR/authenticate flow exactly once.
    pub auth_started: bool,
}

impl ChessAuthScreen {
    pub fn new() -> Self {
        Self {
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
        Self::new()
    }
}

// ─── SettingsScreen ───────────────────────────────────────────────────────────
// App-level settings landing page. Currently hosts only the "Check for updates"
// entry point, but is a natural home for future toggles (telemetry, board
// orientation, etc.) without further restructuring HomeScreen.

pub struct SettingsScreen {
    pub check_update_button: Button,
    pub back_button: Button,
}

impl SettingsScreen {
    pub fn new() -> Self {
        const BTN_W: u16 = 600;
        const BTN_H: u16 = 120;
        const CENTER_X: i16 = 1072 / 2;
        const CENTER_Y: i16 = 1448 / 2;
        Self {
            check_update_button: Button::new(
                CENTER_X - BTN_W as i16 / 2,
                CENTER_Y - BTN_H as i16 / 2,
                BTN_W,
                BTN_H,
                String::from("Check for updates"),
                40.0,
                true,
            ),
            back_button: Button::new(
                CENTER_X - BTN_W as i16 / 2,
                CENTER_Y * 2 - (BTN_H as i16 - 20) - 32,
                BTN_W,
                BTN_H - 20,
                String::from("back"),
                40.0,
                true,
            ),
        }
    }
}

impl Default for SettingsScreen {
    fn default() -> Self {
        Self::new()
    }
}

// ─── UpdateScreen ─────────────────────────────────────────────────────────────
// Drives the check → (up-to-date | available → apply → applied | failed) state
// machine. The check is kicked off automatically on first render; the Apply
// button is only live while `state` is `Available`. Once `Applied`, the action
// button doubles as a Quit affordance — KUAL relaunches the app on next tap of
// the menu entry, picking up the new binary.

pub enum UpdateState {
    Checking,
    UpToDate,
    Available(UpdateInfo),
    Downloading,
    Applied,
    Failed(String),
}

pub struct UpdateScreen {
    pub state: UpdateState,
    pub action_button: Button,
    pub back_button: Button,
    // First-render flag: kick the check exactly once.
    pub check_started: bool,
}

impl UpdateScreen {
    pub fn new() -> Self {
        const BTN_W: u16 = 600;
        const BTN_H: u16 = 120;
        const CENTER_X: i16 = 1072 / 2;
        const CENTER_Y: i16 = 1448 / 2;
        Self {
            state: UpdateState::Checking,
            action_button: Button::new(
                CENTER_X - BTN_W as i16 / 2,
                CENTER_Y + BTN_H as i16,
                BTN_W,
                BTN_H,
                String::from("…"),
                40.0,
                true,
            ),
            back_button: Button::new(
                CENTER_X - BTN_W as i16 / 2,
                CENTER_Y * 2 - (BTN_H as i16 - 20) - 32,
                BTN_W,
                BTN_H - 20,
                String::from("back"),
                40.0,
                true,
            ),
            check_started: false,
        }
    }
}

impl Default for UpdateScreen {
    fn default() -> Self {
        Self::new()
    }
}

// ─── PuzzleScreen ─────────────────────────────────────────────────────────────
// Plays a Lichess puzzle. Unlike ChessGameScreen there is no stream: the puzzle
// is a self-contained position (`PuzzleSession`) and every move is validated
// locally against the scripted solution. Reuses the regular BoardWidget; the
// sidebar is the puzzle-specific one.

pub struct PuzzleScreen {
    pub board: BoardWidget,
    pub sidebar: PuzzleSidebarWidget,
    // None until the first puzzle has loaded (or while a reload is in flight).
    pub session: Option<PuzzleSession>,
    // OAuth token for authenticated `/api/puzzle/next` requests. None when the
    // user isn't signed in — daily/next still work anonymously.
    pub token: Option<TokenInfo>,
    // Last-used `/next` parameters; seeded into the settings screen and reused
    // by the sidebar's "Next puzzle" button.
    pub params: PuzzleParams,
    /// Shared app configuration. Seeds `params` at construction (so the
    /// user's last confirmed choices survive re-entering the puzzle flow) and
    /// is handed to the settings screen for persisting new choices.
    pub config: Arc<RwLock<AppConfig>>,
    // First-render guard: kick the daily-puzzle fetch exactly once.
    pub load_started: bool,
}

impl PuzzleScreen {
    pub fn new(token: Option<TokenInfo>, config: Arc<RwLock<AppConfig>>) -> Self {
        let params = config.read().map(|c| c.puzzle).unwrap_or_default();
        Self {
            board: BoardWidget::new(Rectangle::new(0, 0, 1072, 1072)),
            sidebar: PuzzleSidebarWidget::new(Rectangle::new(0, 1072, 1072, 376)),
            session: None,
            token,
            params,
            config,
            load_started: false,
        }
    }
}

// ─── PuzzleSettingsScreen ─────────────────────────────────────────────────────
// Configures the `/api/puzzle/next` request. Difficulty / phase / color are
// tap-to-cycle buttons; "Get new puzzle" hands the chosen params back to
// PuzzleScreen, which performs the fetch.

pub struct PuzzleSettingsScreen {
    /// Working copy of the `/next` parameters. Seeded from the shared config;
    /// edits stay local until "Get new puzzle" persists them there.
    pub params: PuzzleParams,
    /// Shared app configuration the confirmed `params` are written back to.
    pub config: Arc<RwLock<AppConfig>>,
    pub difficulty_button: Button,
    pub phase_button: Button,
    pub color_button: Button,
    pub fetch_button: Button,
    pub back_button: Button,
}

impl PuzzleSettingsScreen {
    pub fn new(config: Arc<RwLock<AppConfig>>) -> Self {
        const BTN_W: u16 = 760;
        const BTN_H: u16 = 110;
        const CENTER_X: i16 = 1072 / 2;
        let x = CENTER_X - BTN_W as i16 / 2;
        let stride = BTN_H as i16 + 24;
        let params = config.read().map(|c| c.puzzle).unwrap_or_default();
        Self {
            params,
            config,
            difficulty_button: Button::new(
                x,
                320,
                BTN_W,
                BTN_H,
                format!("Difficulty: {}", params.difficulty.label()),
                38.0,
                true,
            ),
            phase_button: Button::new(
                x,
                320 + stride,
                BTN_W,
                BTN_H,
                format!("Phase: {}", params.phase.label()),
                38.0,
                true,
            ),
            color_button: Button::new(
                x,
                320 + 2 * stride,
                BTN_W,
                BTN_H,
                format!("Color: {}", params.color.label()),
                38.0,
                true,
            ),
            fetch_button: Button::new(
                x,
                320 + 3 * stride + 60,
                BTN_W,
                BTN_H,
                String::from("Get new puzzle"),
                38.0,
                true,
            ),
            back_button: Button::new(
                x,
                1448 - BTN_H as i16 - 40,
                BTN_W,
                BTN_H - 20,
                String::from("Back"),
                38.0,
                true,
            ),
        }
    }
}

// ─── GameActionsScreen ────────────────────────────────────────────────────────
// Reached from the live-game sidebar's settings button. Hosts the irreversible
// game actions — resign and abort — that aren't part of normal board play.

pub struct GameActionsScreen {
    pub app: ChessApp,
    pub resign_button: Button,
    pub abort_button: Button,
    pub back_button: Button,
}

impl GameActionsScreen {
    pub fn new(app: ChessApp) -> Self {
        const BTN_W: u16 = 600;
        const BTN_H: u16 = 120;
        const CENTER_X: i16 = 1072 / 2;
        const CENTER_Y: i16 = 1448 / 2;
        let x = CENTER_X - BTN_W as i16 / 2;
        Self {
            app,
            resign_button: Button::new(
                x,
                CENTER_Y - BTN_H as i16 - 10,
                BTN_W,
                BTN_H,
                String::from("Resign"),
                40.0,
                true,
            ),
            abort_button: Button::new(
                x,
                CENTER_Y + 10,
                BTN_W,
                BTN_H,
                String::from("Abort"),
                40.0,
                true,
            ),
            back_button: Button::new(
                x,
                CENTER_Y * 2 - (BTN_H as i16 - 20) - 32,
                BTN_W,
                BTN_H - 20,
                String::from("back"),
                40.0,
                true,
            ),
        }
    }
}
