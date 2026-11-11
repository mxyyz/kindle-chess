use crate::models::board_api::Turn;
use crate::ui::events::{AppEvent, Rectangle, RectangleExt, TouchEvent, TouchKind};
use crate::ui::renderer::{DrawColor, Renderer};
use crate::ui::widgets::icon_button::{ICON_BUTTON_SIZE, Icon, IconButton, load_icon};
use log::info;

/// Sidebar shown below the board on the live-game screen. Displays the opponent
/// and whose turn it is, with buttons: status-bar toggle (top.png, top-left,
/// above the back-nav button), back-nav (bottom-left), settings (right).
pub struct SidebarWidget {
    area: Rectangle,
    /// UI locale used for the status line; refreshed by the owning screen.
    pub locale: String,
    top_button: IconButton,
    back_button: IconButton,
    settings_button: IconButton,
    // Driven by `set_turn` from the game-state stream events arriving on
    // ChessGameScreen. Read by `render` to draw the status line.
    turn_status: String,
    // Opponent display name ("AI" / a username), set from the GameFull event.
    opponent: String,
    // True after the server rejected an illegal move; the status line then
    // shows "Try something else" with an x icon until the next turn update.
    rejected: bool,
    x_icon: Option<Icon>,
    // Partial-render bands and the content last painted into each. A band is
    // cleared and repainted only when its content differs from the snapshot,
    // so routine board taps redraw the screen but leave the sidebar — and the
    // e-ink panel under it — untouched. Each band is centred and content-width
    // so its clear stays well clear of the edge-mounted buttons.
    opponent_band: Rectangle,
    status_band: Rectangle,
    needs_full_repaint: bool,
    drawn_opponent: Option<String>,
    drawn_status: Option<(bool, String)>,
}

impl SidebarWidget {
    pub fn new(area: Rectangle) -> Self {
        const MARGIN: i16 = 24;
        // Left column: status-bar toggle on top, back-nav below it.
        let top_btn_y = area.y + MARGIN;
        let btn_y = top_btn_y + ICON_BUTTON_SIZE as i16 + 8;
        // Dynamic text bands are centred and only as wide as their content can
        // get. A narrow, centred band leaves a wide gap to the edge-mounted
        // buttons, so the e-ink refresh of a band never disturbs one — the
        // buttons are static and painted only on a full repaint.
        let centered = |w: u16| area.x + (area.width as i16 - w as i16) / 2;
        Self {
            area,
            locale: crate::i18n::DEFAULT_LOCALE.to_string(),
            top_button: IconButton::new(area.x + MARGIN, top_btn_y, "top.png"),
            back_button: IconButton::new(area.x + MARGIN, btn_y, "back-nav.png"),
            settings_button: IconButton::new(
                area.x + area.width as i16 - ICON_BUTTON_SIZE as i16 - MARGIN,
                btn_y,
                "settings.png",
            )
            .with_glyph_size(90),
            turn_status: crate::i18n::tr(crate::i18n::DEFAULT_LOCALE, "common-loading"),
            opponent: String::new(),
            rejected: false,
            x_icon: load_icon("x.png"),
            opponent_band: Rectangle::new(centered(640), area.y + 62, 640, 56),
            status_band: Rectangle::new(centered(560), area.y + 150, 560, 60),
            needs_full_repaint: true,
            drawn_opponent: None,
            drawn_status: None,
        }
    }

    /// Refresh the UI locale (called by the owning screen each render).
    pub fn set_locale(&mut self, loc: &str) {
        self.locale = loc.to_string();
    }

    pub fn set_turn(&mut self, turn: Turn, loc: &str) {
        use crate::i18n::t;
        // A fresh turn update supersedes any rejected-move indication.
        self.rejected = false;
        self.turn_status = match turn {
            Turn::Playing => t(loc, "game-turn-playing", &[]),
            Turn::Waiting => t(loc, "game-turn-waiting", &[]),
            Turn::Over { winner: Some(w) } => t(loc, "game-won", &[("winner", &w)]),
            Turn::Over { winner: None } => t(loc, "game-over", &[]),
        };
    }

    /// Set the opponent's display name (a username, or "AI").
    pub fn set_opponent(&mut self, name: String) {
        self.opponent = name;
    }

    /// Flag the last move as rejected by the server (illegal — e.g. it left the
    /// king in check). Cleared by the next `set_turn`.
    pub fn set_move_rejected(&mut self) {
        self.rejected = true;
    }

    pub fn handle_touch(&self, touch: &TouchEvent) -> Option<AppEvent> {
        if touch.kind != TouchKind::Up {
            return None;
        }
        if self.top_button.contains(touch.x, touch.y) {
            info!("Top-bar (status bar) button pressed");
            return Some(AppEvent::ToggleTopBar);
        }
        if self.back_button.contains(touch.x, touch.y) {
            info!("Back button pressed");
            return Some(AppEvent::ExitToMenu);
        }
        if self.settings_button.contains(touch.x, touch.y) {
            info!("Game-actions button pressed");
            return Some(AppEvent::OpenGameActions);
        }
        None
    }

    /// Force a full repaint on the next `render` — required after another
    /// screen has drawn over the sidebar's area.
    pub fn invalidate(&mut self) {
        self.needs_full_repaint = true;
    }

    pub fn render(&mut self, renderer: &mut Renderer) -> Result<(), Box<dyn std::error::Error>> {
        let full = self.needs_full_repaint;
        if full {
            renderer.draw_rectangle(self.area, DrawColor::White, true)?;
            renderer.draw_rectangle(self.area, DrawColor::Black, false)?;
        }

        // Opponent line — repainted only when the opponent name changes.
        if full || self.drawn_opponent.as_deref() != Some(self.opponent.as_str()) {
            if !full {
                renderer.draw_rectangle(self.opponent_band, DrawColor::White, true)?;
            }
            self.draw_opponent_line(renderer)?;
            self.drawn_opponent = Some(self.opponent.clone());
        }

        // Status line — repainted only when the turn status or the rejected
        // flag changes.
        let status_key = (self.rejected, self.turn_status.clone());
        if full || self.drawn_status.as_ref() != Some(&status_key) {
            if !full {
                renderer.draw_rectangle(self.status_band, DrawColor::White, true)?;
            }
            self.draw_status_line(renderer)?;
            self.drawn_status = Some(status_key);
        }

        // Buttons are static — drawn once per full repaint; the bands above
        // are sized to never overlap them.
        if full {
            self.top_button.draw(renderer)?;
            self.back_button.draw(renderer)?;
            self.settings_button.draw(renderer)?;
            self.needs_full_repaint = false;
        }
        Ok(())
    }

    /// Draw the opponent line ("vs <name>"), centred near the top. Caller has
    /// already cleared the opponent band.
    fn draw_opponent_line(
        &self,
        renderer: &mut Renderer,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.opponent.is_empty() {
            return Ok(());
        }
        let size_px = 36.0;
        let line = format!("vs {}", self.opponent);
        let (tw, _) = renderer.measure_text(&line, size_px);
        renderer.draw_text(
            self.area.x + (self.area.width as i16 - tw as i16) / 2,
            self.area.y + 70,
            &line,
            size_px,
            DrawColor::Black,
        )?;
        Ok(())
    }

    /// Draw the status line: normally the turn status, but an x icon + "Try
    /// something else" (matching the puzzle screen) when the last move was
    /// rejected. Caller has already cleared the status band.
    fn draw_status_line(&self, renderer: &mut Renderer) -> Result<(), Box<dyn std::error::Error>> {
        let size_px = 32.0;
        let status_y = self.area.y + 160;
        if self.rejected {
            let text = crate::i18n::tr(&self.locale, "game-try-again");
            let (tw, th) = renderer.measure_text(&text, size_px);
            let icon_size: u16 = 48;
            let icon_gap: i16 = 14;
            let icon_w = if self.x_icon.is_some() {
                icon_size as i16 + icon_gap
            } else {
                0
            };
            let block_x = self.area.x + (self.area.width as i16 - (tw as i16 + icon_w)) / 2;
            if let Some(icon) = self.x_icon.as_ref() {
                renderer.draw_image_alpha(
                    block_x,
                    status_y + (th as i16 - icon_size as i16) / 2,
                    icon_size,
                    icon_size,
                    icon,
                    DrawColor::White,
                )?;
            }
            renderer.draw_text(block_x + icon_w, status_y, &text, size_px, DrawColor::Black)?;
        } else {
            let (tw, _) = renderer.measure_text(&self.turn_status, size_px);
            renderer.draw_text(
                self.area.x + (self.area.width as i16 - tw as i16) / 2,
                status_y,
                &self.turn_status,
                size_px,
                DrawColor::Black,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::layout;

    /// Same contract as the board: after the screensaver drew over the sidebar,
    /// invalidate() must make the next render a full repaint.
    #[test]
    fn invalidate_forces_full_repaint() {
        let mut sidebar = SidebarWidget::new(Rectangle::new(
            0,
            layout::SIDEBAR_Y,
            layout::SIDEBAR_W,
            layout::SIDEBAR_H,
        ));
        assert!(sidebar.needs_full_repaint); // fresh widget paints fully once
        sidebar.needs_full_repaint = false;
        sidebar.invalidate();
        assert!(sidebar.needs_full_repaint);
    }
}
