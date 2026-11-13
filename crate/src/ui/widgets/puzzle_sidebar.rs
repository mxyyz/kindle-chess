use log::info;

use crate::models::puzzle::PuzzleStatus;
use crate::ui::events::{AppEvent, Rectangle, RectangleExt, TouchEvent, TouchKind};
use crate::ui::renderer::{DrawColor, Renderer};
use crate::ui::widgets::Button;
use crate::ui::widgets::icon_button::{ICON_BUTTON_SIZE, Icon, IconButton, load_icon};

/// Which glyph, if any, precedes the status line.
#[derive(Clone, Copy, PartialEq)]
enum StatusIcon {
    None,
    Check,
    Cross,
}

/// Sidebar shown below the board on the puzzle screen. Displays the move-result
/// status line (icon + text), the puzzle's difficulty rating and themes, and a
/// button row: back-nav (left), Next puzzle (centre), settings (right). A hint
/// button sits stacked directly above the settings button.
pub struct PuzzleSidebarWidget {
    area: Rectangle,
    /// UI locale for status strings; refreshed by the owning screen.
    pub locale: String,
    back_button: IconButton,
    hint_button: IconButton,
    settings_button: IconButton,
    next_button: Button,
    status_text: String,
    status_icon: StatusIcon,
    // Puzzle metadata — populated once a puzzle has loaded.
    rating: Option<i32>,
    themes: String,
    check_icon: Option<Icon>,
    x_icon: Option<Icon>,
    // Partial-render bands and the content last painted into each. A band is
    // cleared and repainted only when its content differs from the snapshot,
    // so routine board taps redraw the screen but leave the sidebar — and the
    // e-ink panel under it — untouched. Each band is centred and content-width
    // so its clear stays well clear of the edge-mounted buttons.
    status_band: Rectangle,
    info_band: Rectangle,
    needs_full_repaint: bool,
    drawn_status: Option<(String, StatusIcon)>,
    drawn_info: Option<(Option<i32>, String)>,
}

impl PuzzleSidebarWidget {
    pub fn new(area: Rectangle) -> Self {
        const MARGIN: i16 = 24;
        const NEXT_W: u16 = 400;
        const NEXT_H: u16 = 84;
        // Vertical gap between the stacked hint and settings buttons.
        const STACK_GAP: i16 = 12;
        let btn_y = area.y + area.height as i16 - ICON_BUTTON_SIZE as i16 - 18;
        let next_x = area.x + (area.width as i16 - NEXT_W as i16) / 2;
        // Centre the (shorter) text button vertically against the icon buttons.
        let next_y = btn_y + (ICON_BUTTON_SIZE as i16 - NEXT_H as i16) / 2;
        // Right-edge column: settings on the bottom button row, hint stacked
        // directly above it.
        let right_x = area.x + area.width as i16 - ICON_BUTTON_SIZE as i16 - MARGIN;
        // Dynamic text bands are centred and content-width — narrow enough to
        // leave a wide gap to the edge-mounted buttons (the back-nav on the
        // left, the hint/settings stack on the right), so an e-ink refresh of a
        // band never disturbs one.
        let centered = |w: u16| area.x + (area.width as i16 - w as i16) / 2;
        Self {
            area,
            locale: crate::i18n::DEFAULT_LOCALE.to_string(),
            back_button: IconButton::new(area.x + MARGIN, btn_y, "back-nav.png"),
            hint_button: IconButton::new(
                right_x,
                btn_y - ICON_BUTTON_SIZE as i16 - STACK_GAP,
                "hint.png",
            ),
            settings_button: IconButton::new(right_x, btn_y, "settings.png").with_glyph_size(90),
            next_button: Button::new(
                next_x,
                next_y,
                NEXT_W,
                NEXT_H,
                crate::i18n::tr(crate::i18n::DEFAULT_LOCALE, "puzzle-next"),
                34.0,
                true,
            ),
            status_text: crate::i18n::tr(crate::i18n::DEFAULT_LOCALE, "common-loading"),
            status_icon: StatusIcon::None,
            rating: None,
            themes: String::new(),
            check_icon: load_icon("correct.png"),
            x_icon: load_icon("x.png"),
            status_band: Rectangle::new(centered(600), area.y + 32, 600, 72),
            info_band: Rectangle::new(centered(600), area.y + 120, 600, 92),
            needs_full_repaint: true,
            drawn_status: None,
            drawn_info: None,
        }
    }

    /// Update the status line from a puzzle status. Drives the check/cross icon
    /// and the "Correct move" / "Try something else" / "You solved the puzzle"
    /// text.
    /// Refresh the UI locale and the static button labels (called by the
    /// owning screen each render).
    pub fn set_locale(&mut self, loc: &str) {
        self.locale = loc.to_string();
        self.next_button.label = crate::i18n::tr(loc, "puzzle-next");
    }

    pub fn set_status(&mut self, status: &PuzzleStatus) {
        let loc = self.locale.clone();
        let (key, icon) = match status {
            PuzzleStatus::Solving => ("puzzle-status-solving", StatusIcon::None),
            // "Correct move" means the puzzle continues; "Finished Puzzle" only
            // shows once the whole solution has been played.
            PuzzleStatus::Correct => ("puzzle-status-correct", StatusIcon::Check),
            PuzzleStatus::Wrong => ("puzzle-status-wrong", StatusIcon::Cross),
            PuzzleStatus::Solved => ("puzzle-status-solved", StatusIcon::Check),
        };
        self.status_text = crate::i18n::tr(&loc, key);
        self.status_icon = icon;
    }

    /// Set a plain status message with no icon (loading / error states).
    pub fn set_message(&mut self, message: &str) {
        self.status_text = message.to_string();
        self.status_icon = StatusIcon::None;
    }

    /// Populate the rating + themes block shown under the status line. The
    /// themes are capped so the joined line stays within the centred info band.
    pub fn set_puzzle_info(&mut self, rating: i32, themes: &[String], status: &PuzzleStatus) {
        const MAX_THEMES: usize = 4;
        let (new_rating, new_themes) = match status {
            PuzzleStatus::Solving => (None, String::new()),
            PuzzleStatus::Solved => (
                Some(rating),
                themes
                    .iter()
                    .take(MAX_THEMES)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" · "),
            ),
            _ => (self.rating, self.themes.clone()),
        };
        if (new_rating, &new_themes) != (self.rating, &self.themes) {
            // The info text is centered across the full sidebar width but the
            // band-clear path only whites a 600px rect — wider text (e.g. four
            // themes) would leave glyph residue at the sides. Info changes are
            // rare (load / solve / next puzzle), so repaint the whole sidebar.
            self.needs_full_repaint = true;
            self.rating = new_rating;
            self.themes = new_themes;
        }
    }

    pub fn handle_touch(&self, touch: &TouchEvent) -> Option<AppEvent> {
        if touch.kind != TouchKind::Up {
            return None;
        }
        if self.back_button.contains(touch.x, touch.y) {
            info!("Puzzle back button pressed");
            return Some(AppEvent::ExitToMenu);
        }
        if self.hint_button.contains(touch.x, touch.y) {
            info!("Puzzle hint button pressed");
            return Some(AppEvent::ShowPuzzleHint);
        }
        if self.settings_button.contains(touch.x, touch.y) {
            info!("Puzzle settings button pressed");
            return Some(AppEvent::OpenPuzzleSettings);
        }
        if self.next_button.rect.contains(touch.x, touch.y) {
            info!("Next-puzzle button pressed");
            return Some(AppEvent::NextPuzzle);
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

        // Status line — repainted only when its text or icon changes.
        let status_key = (self.status_text.clone(), self.status_icon);
        if full || self.drawn_status.as_ref() != Some(&status_key) {
            if !full {
                renderer.draw_rectangle(self.status_band, DrawColor::White, true)?;
            }
            self.draw_status_line(renderer)?;
            self.drawn_status = Some(status_key);
        }

        // Rating + themes — repainted only when the puzzle metadata changes.
        let info_key = (self.rating, self.themes.clone());
        if full || self.drawn_info.as_ref() != Some(&info_key) {
            if !full {
                renderer.draw_rectangle(self.info_band, DrawColor::White, true)?;
            }
            self.draw_info_block(renderer)?;
            self.drawn_info = Some(info_key);
        }

        // Buttons are static — drawn once per full repaint; the bands above
        // are sized to never overlap them.
        if full {
            self.back_button.draw(renderer)?;
            self.hint_button.draw(renderer)?;
            self.settings_button.draw(renderer)?;
            self.next_button.draw(renderer)?;
            self.needs_full_repaint = false;
        }
        Ok(())
    }

    /// Draw the status line: optional icon + text, horizontally centred as one
    /// block. Caller has already cleared the status band.
    fn draw_status_line(&self, renderer: &mut Renderer) -> Result<(), Box<dyn std::error::Error>> {
        let status_size = 40.0;
        let (tw, th) = renderer.measure_text(&self.status_text, status_size);
        let icon_size: u16 = 56;
        let icon_gap: i16 = 16;
        let icon = match self.status_icon {
            StatusIcon::Check => self.check_icon.as_ref(),
            StatusIcon::Cross => self.x_icon.as_ref(),
            StatusIcon::None => None,
        };
        let icon_w = if icon.is_some() {
            icon_size as i16 + icon_gap
        } else {
            0
        };
        let block_x = self.area.x + (self.area.width as i16 - (tw as i16 + icon_w)) / 2;
        let status_y = self.area.y + 44;
        if let Some(icon) = icon {
            renderer.draw_image_alpha(
                block_x,
                status_y + (th as i16 - icon_size as i16) / 2,
                icon_size,
                icon_size,
                icon,
                DrawColor::White,
            )?;
        }
        renderer.draw_text(
            block_x + icon_w,
            status_y,
            &self.status_text,
            status_size,
            DrawColor::Black,
        )?;
        Ok(())
    }

    /// Draw the rating + themes block — nothing until a puzzle has loaded.
    /// Caller has already cleared the info band.
    fn draw_info_block(&self, renderer: &mut Renderer) -> Result<(), Box<dyn std::error::Error>> {
        let Some(rating) = self.rating else {
            return Ok(());
        };
        let info_size = 28.0;
        let rating_line = format!("Puzzle rating: {}", rating);
        let (rw, _) = renderer.measure_text(&rating_line, info_size);
        renderer.draw_text(
            self.area.x + (self.area.width as i16 - rw as i16) / 2,
            self.area.y + 130,
            &rating_line,
            info_size,
            DrawColor::Black,
        )?;
        if !self.themes.is_empty() {
            let theme_size = 24.0;
            let (thw, _) = renderer.measure_text(&self.themes, theme_size);
            renderer.draw_text(
                self.area.x + (self.area.width as i16 - thw as i16) / 2,
                self.area.y + 172,
                &self.themes,
                theme_size,
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

    fn widget() -> PuzzleSidebarWidget {
        PuzzleSidebarWidget::new(Rectangle::new(
            0,
            layout::SIDEBAR_Y,
            layout::SIDEBAR_W,
            layout::SIDEBAR_H,
        ))
    }

    #[test]
    fn info_change_marks_full_repaint_but_status_change_does_not() {
        let mut w = widget();
        w.needs_full_repaint = false; // simulate post-first-render state

        // Status-only update keeps the cheap band-diff path.
        w.set_status(&PuzzleStatus::Wrong);
        assert!(!w.needs_full_repaint);

        // Showing the solved info is a change → must fully repaint, because the
        // themes text can be wider than the 600px info band and a band-sized
        // white rect would leave glyph residue at the sides.
        w.set_puzzle_info(1500, &["tactics".to_string()], &PuzzleStatus::Solved);
        assert!(w.needs_full_repaint);
        assert_eq!(w.rating, Some(1500));
        assert_eq!(w.themes, "tactics");

        // Re-setting identical info is not a change → no repaint.
        w.needs_full_repaint = false;
        w.set_puzzle_info(1500, &["tactics".to_string()], &PuzzleStatus::Solved);
        assert!(!w.needs_full_repaint);

        // Clearing the info (next puzzle pressed / new puzzle loading) is a
        // change → full repaint, and the fields actually clear.
        w.set_puzzle_info(0, &[], &PuzzleStatus::Solving);
        assert!(w.needs_full_repaint);
        assert_eq!(w.rating, None);
        assert!(w.themes.is_empty());
    }
}
