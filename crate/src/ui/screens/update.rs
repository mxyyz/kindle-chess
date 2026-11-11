use std::sync::{Arc, RwLock, mpsc::Sender};

use log::{info, warn};

use crate::i18n::{self, tr};

use crate::ui::layout;
use crate::{
    api::{
        github::{UpdateInfo, check_for_update},
        update::{apply_update, apply_update_v2},
    },
    models::ui::{Display, Screen, Transition},
    ui::{
        events::{AppEvent, RectangleExt, TouchKind},
        renderer::DrawColor,
        widgets::Button,
    },
    utils::config::AppConfig,
    version,
};

// ─── UpdateScreen ─────────────────────────────────────────────────────────────
// Drives the check → (up-to-date | available → apply → applied | failed) state
// machine. The check is kicked off automatically on first render; the Apply
// button is only live while `state` is `Available`. Once `Applied`, the action
// button doubles as a Quit affordance — KUAL relaunches the app on next tap of
// the menu entry, picking up the new binary.

pub enum UpdateState {
    Checking,
    UpToDate,
    Available(Box<UpdateInfo>),
    Downloading,
    Applied,
    Failed(String),
}

pub struct UpdateScreen {
    pub config: Arc<RwLock<AppConfig>>,
    pub state: UpdateState,
    pub action_button: Button,
    pub back_button: Button,
    // First-render flag: kick the check exactly once.
    pub check_started: bool,
}

impl UpdateScreen {
    pub fn new(config: Arc<RwLock<AppConfig>>) -> Self {
        const BTN_W: u16 = 600;
        const BTN_H: u16 = 120;
        Self {
            config,
            state: UpdateState::Checking,
            action_button: Button::new(
                layout::CENTER_X - BTN_W as i16 / 2,
                layout::CENTER_Y + BTN_H as i16,
                BTN_W,
                BTN_H,
                String::from("…"),
                40.0,
                true,
            ),
            back_button: Button::new(
                layout::CENTER_X - BTN_W as i16 / 2,
                layout::CENTER_Y * 2 - (BTN_H as i16 - 20) - 32,
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
        Self::new(Arc::new(RwLock::new(AppConfig::load())))
    }
}

// ─── UpdateScreen ─────────────────────────────────────────────────────────────

impl Screen for UpdateScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        // Auto-kick the check on first paint.
        if !self.check_started {
            self.check_started = true;
            kick_update_check(display.event_tx.clone());
        }

        display.renderer.clear(DrawColor::White)?;

        // Title
        let title = tr(&i18n::locale_of(&self.config), "update-title");
        let title_size = 56.0;
        let (tw, _) = display.renderer.measure_text(&title, title_size);
        display.renderer.draw_text(
            (layout::SCREEN_W - tw as i16) / 2,
            120,
            &title,
            title_size,
            DrawColor::Black,
        )?;

        // State-driven body. The action button is only drawn (and only live in
        // the touch handler) when there is something meaningful to do.
        let loc = i18n::locale_of(&self.config);
        let (lines, action_label, action_visible) = match &self.state {
            UpdateState::Checking => (
                vec![tr(&loc, "update-checking")],
                tr(&loc, "update-apply"),
                false,
            ),
            UpdateState::UpToDate => (
                vec![i18n::t(
                    &loc,
                    "update-uptodate",
                    &[("version", version::VERSION)],
                )],
                tr(&loc, "update-apply"),
                false,
            ),
            UpdateState::Available(info) => {
                let size = info.bundle_size.unwrap_or(info.asset_size);
                let mb = size / 1_048_576;
                let mut lines = vec![
                    i18n::t(
                        &loc,
                        "update-available",
                        &[
                            ("current", &info.current.to_string()),
                            ("latest", &info.latest.to_string()),
                        ],
                    ),
                    i18n::t(&loc, "update-size-mb", &[("mb", &mb.to_string())]),
                ];
                if info.is_bundle {
                    lines.push(tr(&loc, "update-bundle-contents"));
                }
                lines.push(String::new());
                lines.push(info.release_name.clone());
                (lines, tr(&loc, "update-apply"), true)
            }
            UpdateState::Downloading => (
                vec![
                    tr(&loc, "update-downloading"),
                    tr(&loc, "update-bundle-hint"),
                ],
                tr(&loc, "update-apply"),
                false,
            ),
            UpdateState::Applied => (
                vec![
                    tr(&loc, "update-applied"),
                    tr(&loc, "update-staged-relaunch"),
                ],
                tr(&loc, "update-quit"),
                true,
            ),
            UpdateState::Failed(err) => (
                vec![tr(&loc, "update-failed"), err.clone()],
                tr(&loc, "update-apply"),
                false,
            ),
        };

        let info_size = 28.0;
        let mut y: i16 = 280;
        for line in &lines {
            let (lw, _) = display.renderer.measure_text(line, info_size);
            let lx = (layout::SCREEN_W - lw as i16) / 2;
            display
                .renderer
                .draw_text(lx, y, line, info_size, DrawColor::Black)?;
            y += 40;
        }

        if action_visible {
            self.action_button.label = action_label.to_string();
            self.action_button.draw(&mut display.renderer)?;
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
            AppEvent::UpdateAvailable(info) => {
                info!("Update available: v{} → v{}", info.current, info.latest);
                self.state = UpdateState::Available(Box::new(info));
                Ok(Transition::Redraw)
            }
            AppEvent::UpdateUpToDate => {
                self.state = UpdateState::UpToDate;
                Ok(Transition::Redraw)
            }
            AppEvent::UpdateCheckFailed(e) => {
                warn!("Update check failed: {}", e);
                self.state = UpdateState::Failed(i18n::t(
                    &i18n::locale_of(&self.config),
                    "update-check-failed",
                    &[("error", &e.to_string())],
                ));
                Ok(Transition::Redraw)
            }
            AppEvent::UpdateApplied => {
                info!("Update applied");
                self.state = UpdateState::Applied;
                Ok(Transition::Redraw)
            }
            AppEvent::UpdateApplyFailed(e) => {
                warn!("Update apply failed: {}", e);
                self.state = UpdateState::Failed(i18n::t(
                    &i18n::locale_of(&self.config),
                    "update-apply-failed",
                    &[("error", &e.to_string())],
                ));
                Ok(Transition::Redraw)
            }

            AppEvent::Touch(touch) => {
                if touch.kind != TouchKind::Up {
                    return Ok(Transition::Stay);
                }
                if self.back_button.rect.contains(touch.x, touch.y) {
                    return Ok(Transition::Pop);
                }
                if self.action_button.rect.contains(touch.x, touch.y) {
                    match &self.state {
                        UpdateState::Available(info) => {
                            let info = (**info).clone();
                            self.state = UpdateState::Downloading;
                            kick_update_apply(info, display.event_tx.clone());
                            return Ok(Transition::Redraw);
                        }
                        UpdateState::Applied => {
                            // KUAL will relaunch the binary the next time the
                            // user taps the menu entry — and exec(2) picks up
                            // the new file at the path we just renamed onto.
                            return Ok(Transition::Quit);
                        }
                        _ => {}
                    }
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

fn kick_update_check(tx: Sender<AppEvent>) {
    tokio::spawn(async move {
        match check_for_update().await {
            Ok(Some(info)) => {
                let _ = tx.send(AppEvent::UpdateAvailable(info));
            }
            Ok(None) => {
                let _ = tx.send(AppEvent::UpdateUpToDate);
            }
            Err(e) => {
                let _ = tx.send(AppEvent::UpdateCheckFailed(e.to_string()));
            }
        }
    });
}

fn kick_update_apply(info: UpdateInfo, tx: Sender<AppEvent>) {
    tokio::spawn(async move {
        // Bundle releases (e05+) go through the staged bundle flow; legacy
        // binary-only releases keep the old single-file path.
        let result = if info.is_bundle {
            apply_update_v2(&info).await
        } else {
            apply_update(&info).await
        };
        match result {
            Ok(()) => {
                let _ = tx.send(AppEvent::UpdateApplied);
            }
            Err(e) => {
                let _ = tx.send(AppEvent::UpdateApplyFailed(e.to_string()));
            }
        }
    });
}
