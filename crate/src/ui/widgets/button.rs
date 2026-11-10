use crate::ui::renderer::DrawColor;
use crate::ui::{
    events::{Rectangle, RectangleExt},
    renderer::Renderer,
};

pub struct Button {
    pub rect: Rectangle,
    pub label: String,
    pub font_size: f32,
    pub outline: bool,
}

impl Button {
    pub fn new(
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        label: String,
        font_size: f32,
        outline: bool,
    ) -> Self {
        Self {
            rect: Rectangle::new(x, y, width, height),
            label,
            font_size,
            outline,
        }
    }
    pub fn draw(&self, renderer: &mut Renderer) -> Result<(), Box<dyn std::error::Error>> {
        renderer.draw_rectangle(self.rect, DrawColor::White, true)?;
        if self.outline {
            renderer.draw_rectangle(self.rect, DrawColor::Black, false)?;
        }
        let label = self.label.as_str();
        let (tw, th) = renderer.measure_text(label, self.font_size);
        let tx = self.rect.x + (self.rect.width as i16 - tw as i16) / 2;
        let ty = self.rect.y + (self.rect.height as i16 - th as i16) / 2;
        renderer.draw_text(tx, ty, label, self.font_size, DrawColor::Black)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_rect_hit_area() {
        let btn = Button::new(50, 100, 200, 60, "Play".into(), 30.0, true);
        assert_eq!(btn.rect.x, 50);
        assert_eq!(btn.rect.y, 100);
        assert_eq!(btn.rect.width, 200);
        assert_eq!(btn.rect.height, 60);
        // Hit test via the public rect field
        assert!(btn.rect.contains(50, 100));
        assert!(btn.rect.contains(249, 159));
        assert!(!btn.rect.contains(250, 100)); // right edge exclusive
        assert!(!btn.rect.contains(50, 160)); // bottom edge exclusive
    }

    #[test]
    fn button_fields_preserved() {
        let btn = Button::new(0, 0, 100, 40, "Resign".into(), 40.0, false);
        assert_eq!(btn.label, "Resign");
        assert_eq!(btn.font_size, 40.0);
        assert!(!btn.outline);
    }
}
