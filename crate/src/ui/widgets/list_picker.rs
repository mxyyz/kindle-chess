//! Reusable modal list-picker overlay. Renders a scrollable list of items
//! on top of the current screen with Select / Cancel buttons. The host
//! screen owns a `ListPicker` and calls `draw` during its render, and
//! `handle_touch` during event handling. On selection the picker emits
//! the chosen item via `PickResult`.

use crate::ui::events::{Rectangle, RectangleExt};
use crate::ui::layout;
use crate::ui::renderer::{DrawColor, Renderer};
use crate::ui::widgets::Button;

/// How the picker resolved a touch interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickResult {
    /// User tapped an item and confirmed with Select.
    Selected(usize),
    /// User tapped Cancel or tapped outside the modal.
    Cancelled,
    /// Touch did not hit the picker — host should handle it.
    NotHandled,
}

/// Modal list-picker overlay.
pub struct ListPicker {
    /// Full-screen modal rect (for outside-tap detection).
    pub modal_rect: Rectangle,
    /// Items to choose from.
    pub items: Vec<String>,
    /// Currently highlighted item index (keyboard/tap highlight).
    pub selected: usize,
    /// First visible item index (scroll offset).
    pub scroll_offset: usize,
    /// Max visible items at once.
    pub max_visible: usize,
    /// Item row height.
    pub row_h: u16,
    /// Select button.
    pub select_button: Button,
    /// Cancel button.
    pub cancel_button: Button,
    /// Optional title drawn at the top of the modal.
    pub title: String,
    /// e07s08: when true, tapping an item immediately selects it (no Select
    /// button needed). When false, item tap only highlights; Select confirms.
    pub tap_to_select: bool,
}

impl ListPicker {
    /// Create a new picker centred on screen.
    pub fn new(items: Vec<String>, title: String) -> Self {
        let modal_w: u16 = 800;
        let modal_h: u16 = 1000;
        let modal_x = (layout::SCREEN_W - modal_w as i16) / 2;
        let modal_y = (layout::SCREEN_H - modal_h as i16) / 2;
        let modal_rect = Rectangle::new(modal_x, modal_y, modal_w, modal_h);

        let row_h: u16 = 80;
        let max_visible = 8usize;
        let select_w: u16 = 280;
        let btn_h: u16 = 80;
        let btn_y = modal_y + modal_h as i16 - btn_h as i16 - 40;
        let select_x = modal_x + 40;
        let cancel_x = modal_x + modal_w as i16 - select_w as i16 - 40;

        Self {
            modal_rect,
            items,
            selected: 0,
            scroll_offset: 0,
            max_visible,
            row_h,
            select_button: Button::new(
                select_x,
                btn_y,
                select_w,
                btn_h,
                "Select".into(),
                32.0,
                true,
            ),
            cancel_button: Button::new(
                cancel_x,
                btn_y,
                select_w,
                btn_h,
                "Cancel".into(),
                32.0,
                true,
            ),
            title,
            tap_to_select: false,
        }
    }

    /// Rect of the item row at visible position `vis_idx` (0-based within
    /// the visible window). Returns None if beyond the modal.
    fn item_rect(&self, vis_idx: usize) -> Rectangle {
        let list_top = self.modal_rect.y + 120; // below title
        let x = self.modal_rect.x + 40;
        let w = self.modal_rect.width - 80;
        let y = list_top + vis_idx as i16 * (self.row_h as i16 + 8);
        Rectangle::new(x, y, w, self.row_h)
    }

    /// Number of visible item rows.
    fn visible_count(&self) -> usize {
        self.items.len().min(self.max_visible)
    }

    /// Handle a touch at (px, py). Returns PickResult.
    pub fn handle_touch(&mut self, px: i16, py: i16) -> PickResult {
        // Outside modal → cancel.
        if !self.modal_rect.contains(px, py) {
            return PickResult::Cancelled;
        }
        // Cancel button.
        if self.cancel_button.rect.contains(px, py) {
            return PickResult::Cancelled;
        }
        // Select button.
        if self.select_button.rect.contains(px, py) {
            return PickResult::Selected(self.selected);
        }
        // Item rows.
        for vis_idx in 0..self.visible_count() {
            let rect = self.item_rect(vis_idx);
            if rect.contains(px, py) {
                let item_idx = self.scroll_offset + vis_idx;
                if item_idx < self.items.len() {
                    self.selected = item_idx;
                    // e07s08: tap-to-select mode returns Selected immediately.
                    if self.tap_to_select {
                        return PickResult::Selected(item_idx);
                    }
                }
                return PickResult::NotHandled;
            }
        }
        PickResult::NotHandled
    }

    /// Scroll so that `selected` is visible.
    pub fn ensure_visible(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + self.max_visible {
            self.scroll_offset = self.selected + 1 - self.max_visible;
        }
    }

    /// Scroll up by one page.
    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    /// Scroll down by one page.
    pub fn scroll_down(&mut self) {
        let max_offset = self.items.len().saturating_sub(self.max_visible);
        if self.scroll_offset < max_offset {
            self.scroll_offset += 1;
        }
    }

    /// Draw the modal overlay.
    pub fn draw(&self, renderer: &mut Renderer) -> Result<(), Box<dyn std::error::Error>> {
        // Modal background (solid white with black border).
        renderer.draw_rectangle(self.modal_rect, DrawColor::White, true)?;
        renderer.draw_rectangle(self.modal_rect, DrawColor::Black, false)?;

        // Title.
        let (tw, _) = renderer.measure_text(&self.title, 36.0);
        let tx = self.modal_rect.x + (self.modal_rect.width as i16 - tw as i16) / 2;
        renderer.draw_text(
            tx,
            self.modal_rect.y + 40,
            &self.title,
            36.0,
            DrawColor::Black,
        )?;

        // Items.
        for vis_idx in 0..self.visible_count() {
            let item_idx = self.scroll_offset + vis_idx;
            if item_idx >= self.items.len() {
                break;
            }
            let rect = self.item_rect(vis_idx);
            let is_selected = item_idx == self.selected;
            if is_selected {
                renderer.draw_rectangle(rect, DrawColor::LightGray, true)?;
            }
            renderer.draw_rectangle(rect, DrawColor::Black, false)?;
            let label = &self.items[item_idx];
            let (_lw, lh) = renderer.measure_text(label, 28.0);
            let lx = rect.x + 16;
            let ly = rect.y + (rect.height as i16 - lh as i16) / 2;
            renderer.draw_text(lx, ly, label, 28.0, DrawColor::Black)?;
        }

        // Scroll indicators.
        if self.scroll_offset > 0 {
            let label = "▲";
            let (lw, _) = renderer.measure_text(label, 24.0);
            let lx = self.modal_rect.x + (self.modal_rect.width as i16 - lw as i16) / 2;
            renderer.draw_text(lx, self.modal_rect.y + 90, label, 24.0, DrawColor::Black)?;
        }
        let max_offset = self.items.len().saturating_sub(self.max_visible);
        if self.scroll_offset < max_offset {
            let label = "▼";
            let (lw, _) = renderer.measure_text(label, 24.0);
            let lx = self.modal_rect.x + (self.modal_rect.width as i16 - lw as i16) / 2;
            let list_bottom =
                self.modal_rect.y + 120 + self.visible_count() as i16 * (self.row_h as i16 + 8);
            renderer.draw_text(lx, list_bottom, label, 24.0, DrawColor::Black)?;
        }

        // Buttons. e07s08: skip Select button in tap-to-select mode.
        if !self.tap_to_select {
            self.select_button.draw(renderer)?;
        }
        self.cancel_button.draw(renderer)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_picker(n: usize) -> ListPicker {
        let items: Vec<String> = (0..n).map(|i| format!("item-{i}")).collect();
        ListPicker::new(items, "Pick one".into())
    }

    #[test]
    fn picker_modal_rect_is_centered() {
        let p = make_picker(3);
        assert_eq!(p.modal_rect.x, (layout::SCREEN_W - 800) / 2);
        assert_eq!(p.modal_rect.y, (layout::SCREEN_H - 1000) / 2);
        assert_eq!(p.modal_rect.width, 800);
        assert_eq!(p.modal_rect.height, 1000);
    }

    #[test]
    fn touch_outside_modal_cancels() {
        let mut p = make_picker(3);
        let r = p.handle_touch(0, 0);
        assert_eq!(r, PickResult::Cancelled);
    }

    #[test]
    fn touch_on_cancel_button_cancels() {
        let mut p = make_picker(3);
        let cx = p.cancel_button.rect.x + 1;
        let cy = p.cancel_button.rect.y + 1;
        let r = p.handle_touch(cx, cy);
        assert_eq!(r, PickResult::Cancelled);
    }

    #[test]
    fn touch_on_select_returns_selected_index() {
        let mut p = make_picker(5);
        p.selected = 2;
        let sx = p.select_button.rect.x + 1;
        let sy = p.select_button.rect.y + 1;
        let r = p.handle_touch(sx, sy);
        assert_eq!(r, PickResult::Selected(2));
    }

    #[test]
    fn touch_on_item_updates_selection() {
        let mut p = make_picker(5);
        // Second visible item.
        let rect = p.item_rect(1);
        let r = p.handle_touch(rect.x + 5, rect.y + 5);
        assert_eq!(r, PickResult::NotHandled);
        assert_eq!(p.selected, 1);
    }

    #[test]
    fn scroll_clamps_to_bounds() {
        let mut p = make_picker(20);
        // scroll_offset starts at 0; scroll_up should stay at 0.
        p.scroll_up();
        assert_eq!(p.scroll_offset, 0);
        // Scroll down several times.
        for _ in 0..30 {
            p.scroll_down();
        }
        assert_eq!(p.scroll_offset, 20 - p.max_visible);
    }

    #[test]
    fn ensure_visible_scrolls_when_needed() {
        let mut p = make_picker(20);
        p.selected = 15;
        p.ensure_visible();
        assert!(p.selected >= p.scroll_offset);
        assert!(p.selected < p.scroll_offset + p.max_visible);
    }

    #[test]
    fn empty_items_does_not_panic() {
        let p = make_picker(0);
        assert_eq!(p.visible_count(), 0);
    }

    #[test]
    fn item_rect_stays_within_modal() {
        let p = make_picker(20);
        for vis_idx in 0..p.visible_count() {
            let r = p.item_rect(vis_idx);
            assert!(r.x >= p.modal_rect.x);
            assert!(r.x + r.width as i16 <= p.modal_rect.x + p.modal_rect.width as i16);
        }
    }

    /// e07s08: tap_to_select=true → item tap returns Selected immediately.
    #[test]
    fn tap_to_select_returns_selected_on_item_tap() {
        let mut p = make_picker(5);
        p.tap_to_select = true;
        let rect = p.item_rect(2); // third item
        let r = p.handle_touch(rect.x + 5, rect.y + 5);
        assert_eq!(r, PickResult::Selected(2));
    }

    /// e07s08: tap_to_select=false (default) → item tap only highlights.
    #[test]
    fn no_tap_to_select_highlights_only() {
        let mut p = make_picker(5);
        let rect = p.item_rect(3);
        let r = p.handle_touch(rect.x + 5, rect.y + 5);
        assert_eq!(r, PickResult::NotHandled);
        assert_eq!(p.selected, 3);
    }
}
