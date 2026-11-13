use std::sync::{Arc, RwLock};

use log::{info, warn};

use crate::i18n::{self, tr};

use crate::ui::layout;
use crate::{
    models::{
        puzzle::PuzzleParams,
        ui::{Display, Screen, Transition},
    },
    ui::{
        events::{AppEvent, RectangleExt, TouchKind},
        renderer::DrawColor,
        widgets::Button,
    },
    utils::config::AppConfig,
};

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
        let x = layout::CENTER_X - BTN_W as i16 / 2;
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
                layout::SCREEN_H - BTN_H as i16 - 40,
                BTN_W,
                BTN_H - 20,
                String::from("Back"),
                38.0,
                true,
            ),
        }
    }
}

// ─── PuzzleSettingsScreen ─────────────────────────────────────────────────────

impl Screen for PuzzleSettingsScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        display.renderer.clear(DrawColor::White)?;

        // Localize all labels every render (cheap lookups; picks up language
        // switches and the current param values).
        let loc = i18n::locale_of(&self.config);
        let title = tr(&loc, "puzzle-settings-title");
        layout::draw_page_title(&mut display.renderer, &title)?;

        self.difficulty_button.label = i18n::t(
            &loc,
            "puzzle-difficulty",
            &[("value", &tr(&loc, self.params.difficulty.i18n_key()))],
        );
        self.phase_button.label = i18n::t(
            &loc,
            "puzzle-phase",
            &[("value", &tr(&loc, self.params.phase.i18n_key()))],
        );
        self.color_button.label = i18n::t(
            &loc,
            "puzzle-color",
            &[("value", &tr(&loc, self.params.color.i18n_key()))],
        );
        self.fetch_button.label = tr(&loc, "puzzle-fetch");
        self.back_button.label = tr(&loc, "settings-back");

        self.difficulty_button.draw(&mut display.renderer)?;
        self.phase_button.draw(&mut display.renderer)?;
        self.color_button.draw(&mut display.renderer)?;
        self.fetch_button.draw(&mut display.renderer)?;
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

                // Tap-to-cycle option buttons.
                if self.difficulty_button.rect.contains(touch.x, touch.y) {
                    self.params.difficulty = self.params.difficulty.next();
                    self.difficulty_button.label = i18n::t(
                        &i18n::locale_of(&self.config),
                        "puzzle-difficulty",
                        &[(
                            "value",
                            &tr(
                                &i18n::locale_of(&self.config),
                                self.params.difficulty.i18n_key(),
                            ),
                        )],
                    );
                    return Ok(Transition::Redraw);
                }
                if self.phase_button.rect.contains(touch.x, touch.y) {
                    self.params.phase = self.params.phase.next();
                    self.phase_button.label = i18n::t(
                        &i18n::locale_of(&self.config),
                        "puzzle-phase",
                        &[(
                            "value",
                            &tr(&i18n::locale_of(&self.config), self.params.phase.i18n_key()),
                        )],
                    );
                    return Ok(Transition::Redraw);
                }
                if self.color_button.rect.contains(touch.x, touch.y) {
                    self.params.color = self.params.color.next();
                    self.color_button.label = i18n::t(
                        &i18n::locale_of(&self.config),
                        "puzzle-color",
                        &[(
                            "value",
                            &tr(&i18n::locale_of(&self.config), self.params.color.i18n_key()),
                        )],
                    );
                    return Ok(Transition::Redraw);
                }

                // Persist the confirmed choices in the shared config so they
                // outlive this screen and seed the next PuzzleScreen, then
                // hand them to PuzzleScreen (top of stack once we pop) to
                // drive the fetch.
                if self.fetch_button.rect.contains(touch.x, touch.y) {
                    info!("Fetching puzzle with {:?}", self.params);
                    match self.config.write() {
                        Ok(mut config) => {
                            config.puzzle = self.params;
                            config.save();
                        }
                        Err(e) => {
                            warn!("Config store poisoned; puzzle params not persisted: {}", e)
                        }
                    }
                    let _ = display
                        .event_tx
                        .send(AppEvent::ApplyPuzzleParams(self.params));
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
