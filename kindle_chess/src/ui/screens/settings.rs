use std::sync::{Arc, RwLock};

use log::{info, warn};

use crate::i18n::{self, tr};

use super::*;

use crate::{
    models::ui::{Display, Screen, Transition},
    ui::{
        events::{AppEvent, RectangleExt, TouchKind},
        renderer::DrawColor,
        widgets::Button,
    },
    utils::config::AppConfig,
    version,
};

// ─── SettingsScreen ───────────────────────────────────────────────────────────
// App-level settings landing page. Currently hosts only the "Check for updates"
// entry point, but is a natural home for future toggles (telemetry, board
// orientation, etc.) without further restructuring HomeScreen.

pub struct SettingsScreen {
    pub config: Arc<RwLock<AppConfig>>,
    pub language_button: Button,
    pub check_update_button: Button,
    pub back_button: Button,
}

impl SettingsScreen {
    pub fn new(config: Arc<RwLock<AppConfig>>) -> Self {
        const BTN_W: u16 = 600;
        const BTN_H: u16 = 120;
        const CENTER_X: i16 = 1072 / 2;
        const CENTER_Y: i16 = 1448 / 2;
        Self {
            config,
            language_button: Button::new(
                CENTER_X - BTN_W as i16 / 2,
                CENTER_Y - BTN_H as i16 / 2 - 140,
                BTN_W,
                BTN_H,
                String::from("Language"),
                40.0,
                true,
            ),
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
        Self::new(Arc::new(RwLock::new(AppConfig::load())))
    }
}

// ─── SettingsScreen ───────────────────────────────────────────────────────────

impl Screen for SettingsScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        display.renderer.clear(DrawColor::White)?;

        // Title + localized button labels
        let loc = i18n::locale_of(&self.config);
        self.language_button.label =
            i18n::t(&loc, "settings-language", &[("lang", &loc.to_uppercase())]);
        self.check_update_button.label = tr(&loc, "settings-check-update");
        self.back_button.label = tr(&loc, "settings-back");
        let title = tr(&loc, "home-settings");
        let title_size = 56.0;
        let (tw, _) = display.renderer.measure_text(&title, title_size);
        display.renderer.draw_text(
            (1072 - tw as i16) / 2,
            120,
            &title,
            title_size,
            DrawColor::Black,
        )?;

        // Version block
        let info_size = 28.0;
        let lines = [
            format!("Version:  {}", version::VERSION),
            format!("Commit:   {}", version::GIT_SHA),
            format!("Built:    {}", version::BUILD_TIMESTAMP),
        ];
        let mut y: i16 = 240;
        for line in &lines {
            display
                .renderer
                .draw_text(120, y, line, info_size, DrawColor::Black)?;
            y += 40;
        }

        self.language_button.draw(&mut display.renderer)?;
        self.check_update_button.draw(&mut display.renderer)?;
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
                if self.language_button.rect.contains(touch.x, touch.y) {
                    // Cycle through the shipped locales: en -> de -> en …
                    let next = {
                        let cur = i18n::locale_of(&self.config);
                        let idx = i18n::LOCALES.iter().position(|l| *l == cur).unwrap_or(0);
                        i18n::LOCALES[(idx + 1) % i18n::LOCALES.len()].to_string()
                    };
                    info!("Language switched to {next}");
                    if let Ok(mut c) = self.config.write() {
                        c.language = next;
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
