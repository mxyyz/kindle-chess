use std::sync::{Arc, RwLock};

use log::{info, warn};

use crate::i18n::{self, tr};

use super::*;

use crate::ui::layout;
use crate::{
    models::{
        chess::ChessApp,
        puzzle::PuzzleParams,
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
    utils::config::{AppConfig, BoardOrientation},
    version,
};

// ─── SettingsContext ──────────────────────────────────────────────────────────
// Which screen opened the SettingsScreen — decides what the primary
// ("This screen") tab shows. e08s03 routes Game/Puzzle settings through here.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsContext {
    /// Home screen — no screen-specific settings; primary tab shows version info.
    #[default]
    General,
    /// Live chess game screen.
    Game,
    /// Daily puzzle screen.
    Puzzle,
}

/// Which tab is currently visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Primary,
    General,
}

// ─── SettingsScreen ───────────────────────────────────────────────────────────
// 2-tab settings screen (e08s02): primary tab = context-specific content,
// secondary tab = shared general settings (language, board orientation,
// auto-update, board labels, check-for-updates).

pub struct SettingsScreen {
    pub config: Arc<RwLock<AppConfig>>,
    pub context: SettingsContext,
    active_tab: SettingsTab,
    // Tab toggle buttons (side by side at top)
    tab_primary_button: Button,
    tab_general_button: Button,
    // General tab buttons (shared across all contexts)
    orientation_button: Button,
    board_labels_button: Button,
    chrome_button: Button,
    chrome_icons: [Option<Icon>; 3],
    // Home-specific primary-tab buttons (context == General)
    language_button: Button,
    auto_update_button: Button,
    check_update_button: Button,
    // Puzzle primary-tab buttons (context == Puzzle)
    puzzle_params: PuzzleParams,
    puzzle_difficulty_button: Button,
    puzzle_phase_button: Button,
    puzzle_color_button: Button,
    puzzle_fetch_button: Button,
    // Game primary-tab buttons (context == Game)
    game_app: Option<ChessApp>,
    resign_button: Button,
    abort_button: Button,
    // Navigation
    back_button: Button,
}

impl SettingsScreen {
    pub fn new(config: Arc<RwLock<AppConfig>>) -> Self {
        Self::with_context(config, SettingsContext::default())
    }

    pub fn with_context(config: Arc<RwLock<AppConfig>>, context: SettingsContext) -> Self {
        Self::with_context_and_app(config, context, None)
    }

    pub fn with_context_and_app(
        config: Arc<RwLock<AppConfig>>,
        context: SettingsContext,
        game_app: Option<ChessApp>,
    ) -> Self {
        const BTN_W: u16 = 760;
        const BTN_H: u16 = 100;
        const TAB_W: u16 = 490;
        const TAB_H: u16 = 80;
        let x = layout::CENTER_X - BTN_W as i16 / 2;
        let tab_y: i16 = 200;
        let tab_primary_x: i16 = 36;
        let tab_general_x: i16 = 36 + TAB_W as i16 + 20;
        let content_y: i16 = 320;
        let stride: i16 = BTN_H as i16 + 28;

        // Default to the General tab when context is General (Home);
        // otherwise default to the Primary ("This screen") tab.
        let active_tab = match context {
            SettingsContext::General => SettingsTab::General,
            _ => SettingsTab::Primary,
        };

        let puzzle_params = config.read().map(|c| c.puzzle).unwrap_or_default();

        Self {
            config,
            context,
            active_tab,
            tab_primary_button: Button::new(
                tab_primary_x,
                tab_y,
                TAB_W,
                TAB_H,
                String::from("This screen"),
                34.0,
                true,
            ),
            tab_general_button: Button::new(
                tab_general_x,
                tab_y,
                TAB_W,
                TAB_H,
                String::from("General"),
                34.0,
                true,
            ),
            // General tab (shared): orientation, board labels, chrome toggle
            orientation_button: Button::new(
                x,
                content_y,
                BTN_W,
                BTN_H,
                String::from("Board orientation"),
                38.0,
                true,
            ),
            board_labels_button: Button::new(
                x,
                content_y + stride,
                BTN_W,
                BTN_H,
                String::from("Board labels"),
                38.0,
                true,
            ),
            chrome_button: Button::new(
                x,
                content_y + 2 * stride,
                BTN_W,
                BTN_H,
                String::from(""),
                38.0,
                true,
            ),
            chrome_icons: [
                load_icon("wifi.png"),
                load_icon("device_settings.png"),
                load_icon("brightness.png"),
            ],
            // Home-specific primary tab: language, auto-update, check update
            language_button: Button::new(
                x,
                content_y,
                BTN_W,
                BTN_H,
                String::from("Language"),
                38.0,
                true,
            ),
            auto_update_button: Button::new(
                x,
                content_y + stride,
                BTN_W,
                BTN_H,
                String::from("Auto-check updates"),
                38.0,
                true,
            ),
            check_update_button: Button::new(
                x,
                content_y + 2 * stride,
                BTN_W,
                BTN_H,
                String::from("Check for updates"),
                38.0,
                true,
            ),
            puzzle_params,
            puzzle_difficulty_button: Button::new(
                x,
                content_y,
                BTN_W,
                BTN_H,
                String::from("Difficulty"),
                38.0,
                true,
            ),
            puzzle_phase_button: Button::new(
                x,
                content_y + stride,
                BTN_W,
                BTN_H,
                String::from("Phase"),
                38.0,
                true,
            ),
            puzzle_color_button: Button::new(
                x,
                content_y + 2 * stride,
                BTN_W,
                BTN_H,
                String::from("Color"),
                38.0,
                true,
            ),
            puzzle_fetch_button: Button::new(
                x,
                content_y + 3 * stride + 40,
                BTN_W,
                BTN_H,
                String::from("Get new puzzle"),
                38.0,
                true,
            ),
            game_app,
            resign_button: Button::new(
                x,
                content_y + stride,
                BTN_W,
                BTN_H,
                String::from("Resign"),
                38.0,
                true,
            ),
            abort_button: Button::new(
                x,
                content_y + 2 * stride,
                BTN_W,
                BTN_H,
                String::from("Abort"),
                38.0,
                true,
            ),
            back_button: Button::new(
                x,
                layout::SCREEN_H - BTN_H as i16 - 40,
                BTN_W,
                BTN_H - 20,
                String::from("back"),
                38.0,
                true,
            ),
        }
    }

    /// Draw the 3 icons (wifi / device_settings / brightness) centred inside
    /// the chrome button, separated by "/" text.
    fn draw_chrome_icons(
        &self,
        renderer: &mut crate::ui::renderer::Renderer,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // If no icons loaded, draw a text fallback so the button isn't blank.
        if self.chrome_icons.iter().all(|i| i.is_none()) {
            let label = "wifi / settings / brightness";
            let (tw, th) = renderer.measure_text(label, 28.0);
            let tx =
                self.chrome_button.rect.x + (self.chrome_button.rect.width as i16 - tw as i16) / 2;
            let ty =
                self.chrome_button.rect.y + (self.chrome_button.rect.height as i16 - th as i16) / 2;
            renderer.draw_text(tx, ty, label, 28.0, DrawColor::Black)?;
            return Ok(());
        }
        let icon_size: i16 = 56;
        let gap: i16 = 12;
        let sep = "/";
        let (sep_w, _) = renderer.measure_text(sep, 32.0);
        let sep_w = sep_w as i16;
        let total_w = 3 * icon_size + 2 * (sep_w + 2 * gap);
        let mut cx =
            self.chrome_button.rect.x + (self.chrome_button.rect.width as i16 - total_w) / 2;
        let cy =
            self.chrome_button.rect.y + (self.chrome_button.rect.height as i16 - icon_size) / 2;
        for (i, icon) in self.chrome_icons.iter().enumerate() {
            if i > 0 {
                renderer.draw_text(cx, cy + icon_size / 2, sep, 32.0, DrawColor::Black)?;
                cx += sep_w + 2 * gap;
            }
            if let Some(icon) = icon {
                renderer.draw_image_alpha(
                    cx,
                    cy,
                    icon_size as u16,
                    icon_size as u16,
                    icon,
                    DrawColor::White,
                )?;
            }
            cx += icon_size;
        }
        Ok(())
    }

    /// Draw a tab button — active tab is filled black with white text.
    fn draw_tab_button(
        &self,
        renderer: &mut crate::ui::renderer::Renderer,
        button: &Button,
        active: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if active {
            renderer.draw_rectangle(button.rect, DrawColor::Black, true)?;
            let (tw, th) = renderer.measure_text(&button.label, button.font_size);
            let tx = button.rect.x + (button.rect.width as i16 - tw as i16) / 2;
            let ty = button.rect.y + (button.rect.height as i16 - th as i16) / 2;
            renderer.draw_text_on_bg(
                tx,
                ty,
                &button.label,
                button.font_size,
                DrawColor::White,
                DrawColor::Black,
            )?;
        } else {
            button.draw(renderer)?;
        }
        Ok(())
    }

    /// Update localized labels for the general-tab buttons from current config.
    fn refresh_general_labels(&mut self) {
        let loc = i18n::locale_of(&self.config);
        self.language_button.label =
            i18n::t(&loc, "settings-language", &[("lang", &loc.to_uppercase())]);

        let (orient_key, orient_val) = {
            let c = self.config.read().unwrap();
            let o = c.general.board_orientation;
            let key = match o {
                BoardOrientation::Auto => "orient-auto",
                BoardOrientation::White => "orient-white",
                BoardOrientation::Black => "orient-black",
            };
            (key, o)
        };
        self.orientation_button.label = i18n::t(
            &loc,
            "settings-orientation",
            &[("value", &tr(&loc, orient_key))],
        );
        let _ = orient_val;

        let auto_update = self
            .config
            .read()
            .map(|c| c.general.auto_update_check)
            .unwrap_or(true);
        let toggle_key = if auto_update {
            "toggle-on"
        } else {
            "toggle-off"
        };
        self.auto_update_button.label = i18n::t(
            &loc,
            "settings-auto-update",
            &[("value", &tr(&loc, toggle_key))],
        );

        let board_labels = self.config.read().map(|c| c.board_labels).unwrap_or(true);
        let bl_key = if board_labels {
            "toggle-on"
        } else {
            "toggle-off"
        };
        self.board_labels_button.label = i18n::t(
            &loc,
            "settings-board-labels",
            &[("value", &tr(&loc, bl_key))],
        );

        self.check_update_button.label = tr(&loc, "settings-check-update");
        self.back_button.label = tr(&loc, "settings-back");
        self.tab_primary_button.label = tr(&loc, "tab-screen");
        self.tab_general_button.label = tr(&loc, "tab-general");

        // Puzzle primary-tab labels
        self.puzzle_difficulty_button.label = i18n::t(
            &loc,
            "puzzle-difficulty",
            &[("value", &tr(&loc, self.puzzle_params.difficulty.i18n_key()))],
        );
        self.puzzle_phase_button.label = i18n::t(
            &loc,
            "puzzle-phase",
            &[("value", &tr(&loc, self.puzzle_params.phase.i18n_key()))],
        );
        self.puzzle_color_button.label = i18n::t(
            &loc,
            "puzzle-color",
            &[("value", &tr(&loc, self.puzzle_params.color.i18n_key()))],
        );
        self.puzzle_fetch_button.label = tr(&loc, "puzzle-fetch");

        // Game primary-tab labels
        self.resign_button.label = tr(&loc, "game-resign");
        self.abort_button.label = tr(&loc, "game-abort");
    }
}

impl Default for SettingsScreen {
    fn default() -> Self {
        Self::new(Arc::new(RwLock::new(AppConfig::load())))
    }
}

// ─── SettingsScreen ───────────────────────────────────────────────────────────

impl Screen for SettingsScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        display.renderer.clear(DrawColor::White)?;

        let loc = i18n::locale_of(&self.config);
        self.refresh_general_labels();
        let title = tr(&loc, "home-settings");
        layout::draw_page_title(&mut display.renderer, &title)?;

        // Tab toggle buttons
        let primary_active = self.active_tab == SettingsTab::Primary;
        let general_active = self.active_tab == SettingsTab::General;
        self.draw_tab_button(
            &mut display.renderer,
            &self.tab_primary_button,
            primary_active,
        )?;
        self.draw_tab_button(
            &mut display.renderer,
            &self.tab_general_button,
            general_active,
        )?;

        match self.active_tab {
            SettingsTab::General => {
                self.orientation_button.draw(&mut display.renderer)?;
                // e08 follow-up: board orientation is deferred — draw a 45°
                // "Coming soon.." label on the button's top-right corner.
                {
                    let label = tr(&loc, "coming-soon");
                    let (tw, th) = display.renderer.measure_text(&label, 32.0);
                    // Clockwise 45°: text runs top-left → bottom-right (\ direction).
                    // Offset: half the text width right, 25% of height up.
                    let bx = self.orientation_button.rect.x
                        + self.orientation_button.rect.width as i16
                        - tw as i16
                        + 2
                        + tw as i16 / 2;
                    let by = self.orientation_button.rect.y - 6 - th as i16 / 4;
                    display.renderer.draw_text_rotated45(
                        bx,
                        by,
                        &label,
                        32.0,
                        DrawColor::Black,
                        DrawColor::White,
                    )?;
                }
                self.board_labels_button.draw(&mut display.renderer)?;
                // Chrome toggle button with 3 icons (wifi / device_settings / brightness)
                self.chrome_button.draw(&mut display.renderer)?;
                self.draw_chrome_icons(&mut display.renderer)?;
            }
            SettingsTab::Primary => match self.context {
                SettingsContext::General => {
                    // Home-specific: language, auto-update, check for updates
                    self.language_button.draw(&mut display.renderer)?;
                    self.auto_update_button.draw(&mut display.renderer)?;
                    self.check_update_button.draw(&mut display.renderer)?;
                    let info_size = 28.0;
                    let lines = [
                        format!("Version:  {}", version::VERSION),
                        format!("Commit:   {}", version::GIT_SHA),
                        format!("Built:    {}", version::BUILD_TIMESTAMP),
                    ];
                    let mut y: i16 = 360 + 3 * (100 + 28);
                    for line in &lines {
                        display
                            .renderer
                            .draw_text(120, y, line, info_size, DrawColor::Black)?;
                        y += 40;
                    }
                }
                SettingsContext::Puzzle => {
                    self.puzzle_difficulty_button.draw(&mut display.renderer)?;
                    self.puzzle_phase_button.draw(&mut display.renderer)?;
                    self.puzzle_color_button.draw(&mut display.renderer)?;
                    self.puzzle_fetch_button.draw(&mut display.renderer)?;
                }
                SettingsContext::Game => {
                    self.resign_button.draw(&mut display.renderer)?;
                    self.abort_button.draw(&mut display.renderer)?;
                }
            },
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
            AppEvent::Touch(touch) => {
                if touch.kind != TouchKind::Up {
                    return Ok(Transition::Stay);
                }

                // Tab toggle
                if self.tab_primary_button.rect.contains(touch.x, touch.y) {
                    self.active_tab = SettingsTab::Primary;
                    return Ok(Transition::Redraw);
                }
                if self.tab_general_button.rect.contains(touch.x, touch.y) {
                    self.active_tab = SettingsTab::General;
                    return Ok(Transition::Redraw);
                }

                // General tab actions (shared across all contexts)
                if self.active_tab == SettingsTab::General {
                    // Board orientation is deferred (coming soon) — button is
                    // visually labelled but has no touch reaction.
                    if self.board_labels_button.rect.contains(touch.x, touch.y) {
                        if let Ok(mut c) = self.config.write() {
                            c.board_labels = !c.board_labels;
                            c.save();
                        }
                        return Ok(Transition::Redraw);
                    }
                    if self.chrome_button.rect.contains(touch.x, touch.y) {
                        // Same as the old top.png sidebar button: toggle Kindle
                        // status bar (PC:T chrome) over the app window.
                        let new_mode = !display.chrome_mode();
                        info!("Chrome toggle from settings — chrome mode -> {}", new_mode);
                        if let Err(e) = display.set_chrome_mode(new_mode) {
                            log::error!("set_chrome_mode failed: {}", e);
                        }
                        return Ok(Transition::Redraw);
                    }
                }

                // Primary tab: Home-specific settings (context == General)
                if self.active_tab == SettingsTab::Primary
                    && self.context == SettingsContext::General
                {
                    if self.language_button.rect.contains(touch.x, touch.y) {
                        let next = {
                            let cur = i18n::locale_of(&self.config);
                            let idx = i18n::LOCALES.iter().position(|l| *l == cur).unwrap_or(0);
                            i18n::LOCALES[(idx + 1) % i18n::LOCALES.len()].to_string()
                        };
                        info!("Language switched to {next}");
                        if let Ok(mut c) = self.config.write() {
                            c.general.language = next;
                            c.save();
                        }
                        return Ok(Transition::Redraw);
                    }
                    if self.auto_update_button.rect.contains(touch.x, touch.y) {
                        if let Ok(mut c) = self.config.write() {
                            c.general.auto_update_check = !c.general.auto_update_check;
                            c.save();
                        }
                        return Ok(Transition::Redraw);
                    }
                    if self.check_update_button.rect.contains(touch.x, touch.y) {
                        info!("Check-for-updates pressed");
                        return Ok(Transition::Push(Box::new(UpdateScreen::new(
                            self.config.clone(),
                        ))));
                    }
                }

                // Primary tab actions (only when primary tab is active)
                if self.active_tab == SettingsTab::Primary {
                    match self.context {
                        SettingsContext::Puzzle => {
                            if self
                                .puzzle_difficulty_button
                                .rect
                                .contains(touch.x, touch.y)
                            {
                                self.puzzle_params.difficulty =
                                    self.puzzle_params.difficulty.next();
                                return Ok(Transition::Redraw);
                            }
                            if self.puzzle_phase_button.rect.contains(touch.x, touch.y) {
                                self.puzzle_params.phase = self.puzzle_params.phase.next();
                                return Ok(Transition::Redraw);
                            }
                            if self.puzzle_color_button.rect.contains(touch.x, touch.y) {
                                self.puzzle_params.color = self.puzzle_params.color.next();
                                return Ok(Transition::Redraw);
                            }
                            if self.puzzle_fetch_button.rect.contains(touch.x, touch.y) {
                                info!("Fetching puzzle with {:?}", self.puzzle_params);
                                if let Ok(mut c) = self.config.write() {
                                    c.puzzle = self.puzzle_params;
                                    c.save();
                                }
                                let _ = display
                                    .event_tx
                                    .send(AppEvent::ApplyPuzzleParams(self.puzzle_params));
                                return Ok(Transition::Pop);
                            }
                        }
                        SettingsContext::Game => {
                            if self.resign_button.rect.contains(touch.x, touch.y) {
                                if let Some(app) = &self.game_app
                                    && let Some(api) = app.online_in_game_api()
                                {
                                    info!("Resigning game");
                                    tokio::spawn(async move {
                                        if let Err(e) = api.resign_game().await {
                                            warn!("resign_game failed: {}", e);
                                        }
                                    });
                                }
                                return Ok(Transition::Pop);
                            }
                            if self.abort_button.rect.contains(touch.x, touch.y) {
                                if let Some(app) = &self.game_app
                                    && let Some(api) = app.online_in_game_api()
                                {
                                    info!("Aborting game");
                                    tokio::spawn(async move {
                                        if let Err(e) = api.abort_game().await {
                                            warn!("abort_game failed: {}", e);
                                        }
                                    });
                                }
                                return Ok(Transition::Pop);
                            }
                        }
                        SettingsContext::General => {}
                    }
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

#[cfg(test)]
mod settings_tests {
    use super::*;

    fn make_screen() -> SettingsScreen {
        let config = Arc::new(RwLock::new(AppConfig::default()));
        SettingsScreen::new(config)
    }

    #[test]
    fn default_context_is_general_tab_active() {
        let s = make_screen();
        assert_eq!(s.context, SettingsContext::General);
        assert_eq!(s.active_tab, SettingsTab::General);
    }

    #[test]
    fn game_context_defaults_to_primary_tab() {
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let s = SettingsScreen::with_context(config, SettingsContext::Game);
        assert_eq!(s.active_tab, SettingsTab::Primary);
    }

    #[test]
    fn puzzle_context_defaults_to_primary_tab() {
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let s = SettingsScreen::with_context(config, SettingsContext::Puzzle);
        assert_eq!(s.active_tab, SettingsTab::Primary);
    }

    #[test]
    fn tab_buttons_do_not_overlap() {
        let s = make_screen();
        let r1 = s.tab_primary_button.rect;
        let r2 = s.tab_general_button.rect;
        assert!(
            r1.x + r1.width as i16 <= r2.x,
            "tab buttons overlap horizontally"
        );
        assert!(r1.y + r1.height as i16 <= r2.y || r1.y == r2.y);
    }

    #[test]
    fn all_buttons_on_canvas() {
        let s = make_screen();
        for b in [
            &s.language_button,
            &s.orientation_button,
            &s.auto_update_button,
            &s.board_labels_button,
            &s.check_update_button,
            &s.chrome_button,
            &s.back_button,
            &s.tab_primary_button,
            &s.tab_general_button,
            &s.puzzle_difficulty_button,
            &s.puzzle_phase_button,
            &s.puzzle_color_button,
            &s.puzzle_fetch_button,
            &s.resign_button,
            &s.abort_button,
        ] {
            assert!(b.rect.x >= 0, "button {} off left edge", b.label);
            assert!(b.rect.y >= 0, "button {} off top edge", b.label);
            assert!(
                b.rect.x + b.rect.width as i16 <= layout::SCREEN_W,
                "button {} off right edge",
                b.label
            );
            assert!(
                b.rect.y + b.rect.height as i16 <= layout::SCREEN_H,
                "button {} off bottom edge",
                b.label
            );
        }
    }

    #[test]
    fn tab_content_buttons_do_not_overlap_within_tab() {
        let s = make_screen();
        // General tab buttons
        let general = [
            &s.orientation_button,
            &s.board_labels_button,
            &s.chrome_button,
        ];
        // Home primary tab buttons
        let home = [
            &s.language_button,
            &s.auto_update_button,
            &s.check_update_button,
        ];
        for group in [&general[..], &home[..]] {
            for i in 0..group.len() {
                for j in (i + 1)..group.len() {
                    let a = group[i].rect;
                    let b = group[j].rect;
                    let overlap = a.x < b.x + b.width as i16
                        && b.x < a.x + a.width as i16
                        && a.y < b.y + b.height as i16
                        && b.y < a.y + a.height as i16;
                    assert!(
                        !overlap,
                        "buttons {} and {} overlap",
                        group[i].label, group[j].label
                    );
                }
            }
        }
    }

    #[test]
    fn back_button_below_all_content() {
        let s = make_screen();
        let lowest = [
            &s.check_update_button,
            &s.chrome_button,
            &s.puzzle_fetch_button,
            &s.abort_button,
        ]
        .iter()
        .map(|b| b.rect.y + b.rect.height as i16)
        .max()
        .unwrap();
        assert!(s.back_button.rect.y >= lowest);
    }

    #[test]
    fn chrome_icons_loaded() {
        let s = make_screen();
        assert_eq!(s.chrome_icons.len(), 3);
        // Icons must actually load from ASSETS_DIR — if any is None the
        // button would render blank (regression: icons were missing from
        // the dev ASSETS_DIR path).
        for (i, icon) in s.chrome_icons.iter().enumerate() {
            assert!(icon.is_some(), "chrome icon slot {i} failed to load");
        }
    }
}
