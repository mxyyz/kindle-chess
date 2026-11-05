//! Tap-only slider widget: picks a single integer in `min..=max` by tapping
//! anywhere on the track. Replaces the +/- spinner on PlayScreen — repeated
//! button taps can trigger the global triple-tap-to-quit, a slider cannot.
//!
//! Hold-to-drag is deliberately not implemented for now; a tap sets the value
//! to the tapped position.

use crate::ui::events::{Rectangle, RectangleExt};
use crate::ui::renderer::{DrawColor, Renderer};

#[derive(Debug, Clone)]
pub struct Slider {
    pub rect: Rectangle,
    pub min: u8,
    pub max: u8,
    value: u8,
    label: String,
}

impl Slider {
    pub fn new(x: i16, y: i16, width: u16, height: u16, min: u8, max: u8, initial: u8, label: &str) -> Self {
        assert!(min < max, "slider range must be non-empty");
        Self {
            rect: Rectangle::new(x, y, width, height),
            min,
            max,
            value: initial.clamp(min, max),
            label: label.to_string(),
        }
    }

    pub fn value(&self) -> u8 {
        self.value
    }

    pub fn set_value(&mut self, v: u8) {
        self.value = v.clamp(self.min, self.max);
    }

    /// Value corresponding to a tap at pixel x on the track.
    pub fn tap_value(&self, px: i16) -> u8 {
        let w = self.rect.width as i16;
        let rel = (px - self.rect.x).clamp(0, w);
        let frac = if w > 0 { rel as f32 / w as f32 } else { 0.0 };
        let v = self.min as f32 + frac * (self.max - self.min) as f32;
        (v.round() as i16).clamp(self.min as i16, self.max as i16) as u8
    }

    pub fn contains(&self, px: i16, py: i16) -> bool {
        self.rect.contains(px, py)
    }

    /// X position of the handle for the current value.
    fn handle_x(&self) -> i16 {
        let w = self.rect.width as i16;
        let frac = (self.value - self.min) as f32 / (self.max - self.min) as f32;
        self.rect.x + (frac * w as f32).round() as i16
    }

    pub fn draw(&self, renderer: &mut Renderer) -> Result<(), Box<dyn std::error::Error>> {
        let mid_y = self.rect.y + self.rect.height as i16 / 2;
        let x0 = self.rect.x + 8;
        let x1 = self.rect.x + self.rect.width as i16 - 8;

        // Label with current value, centred above the track.
        let text = format!("{} ({})", self.label, self.value);
        let (tw, th) = renderer.measure_text(&text, 28.0);
        renderer.draw_text(
            self.rect.x + (self.rect.width as i16 - tw as i16) / 2,
            self.rect.y + 2,
            &text,
            28.0,
            DrawColor::Black,
        )?;
        let _ = th;

        // Track.
        renderer.draw_line(x0, mid_y, x1, mid_y, DrawColor::Black, 3)?;
        // End ticks.
        renderer.draw_line(x0, mid_y - 8, x0, mid_y + 8, DrawColor::Black, 3)?;
        renderer.draw_line(x1, mid_y - 8, x1, mid_y + 8, DrawColor::Black, 3)?;
        // Handle: filled square centred on the value position.
        let hx = self.handle_x();
        renderer.draw_rectangle(
            Rectangle::new(hx - 10, mid_y - 10, 20, 20),
            DrawColor::Black,
            true,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod slider_tests {
    use super::*;

    #[test]
    fn tap_maps_to_range_ends_and_middle() {
        let s = Slider::new(100, 300, 600, 96, 1, 14, 3, "days/move");
        assert_eq!(s.tap_value(100), 1); // left edge
        assert_eq!(s.tap_value(700), 14); // right edge
        assert_eq!(s.tap_value(96), 1); // before track clamps to min
        assert_eq!(s.tap_value(900), 14); // past track clamps to max
        let mid = s.tap_value(400); // centre of 100..700
        assert!((7..=8).contains(&mid), "mid tap should be ~7/8, got {mid}");
    }

    #[test]
    fn set_value_clamps() {
        let mut s = Slider::new(0, 0, 100, 20, 1, 14, 3, "x");
        s.set_value(99);
        assert_eq!(s.value(), 14);
        s.set_value(0);
        assert_eq!(s.value(), 1);
    }

    #[test]
    fn initial_value_clamps() {
        let s = Slider::new(0, 0, 100, 20, 1, 14, 42, "x");
        assert_eq!(s.value(), 14);
    }
}
