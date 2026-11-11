//! Custom screensaver drawn on `AppEvent::DeviceLock` (powerd
//! `goingToScreenSaver`).
//!
//! While pillow is disabled the Kindle's own screensaver never appears — the
//! panel just freezes. So on lock we clear the panel and show the app logo
//! centred; on `DeviceWake` the app loop redraws the top screen as usual.

use crate::ui::renderer::{DrawColor, Renderer};
use crate::ui::widgets::icon_button::{Icon, load_icon};

use crate::ui::layout::SCREEN_H;
/// Canvas dimensions (absolute layout, see ARCHITECTURE.md).
use crate::ui::layout::SCREEN_W;

/// Max logo width on the screensaver.
const LOGO_MAX_W: f32 = 800.0;

/// Decode `logo.png` pre-scaled to its on-screen size, aspect preserved — done
/// per draw is fine here (lock happens rarely), but we still avoid re-scaling
/// a ~1600 px image to huge sizes by capping the width.
fn load_screensaver_logo() -> Option<Icon> {
    let img = load_icon("logo.png")?;
    let lw = img.width() as f32;
    let lh = img.height() as f32;
    let scale = (LOGO_MAX_W / lw).min(1.0);
    Some(image::imageops::resize(
        &img,
        (lw * scale).max(1.0) as u32,
        (lh * scale).max(1.0) as u32,
        image::imageops::FilterType::Triangle,
    ))
}

/// Clear the panel and draw the logo centred — the whole screensaver.
pub fn render_screensaver(renderer: &mut Renderer) -> Result<(), Box<dyn std::error::Error>> {
    renderer.clear(DrawColor::White)?;
    if let Some(logo) = load_screensaver_logo() {
        let dw = logo.width() as i16;
        let dh = logo.height() as i16;
        renderer.draw_image_alpha(
            (SCREEN_W - dw) / 2,
            (SCREEN_H - dh) / 2,
            dw as u16,
            dh as u16,
            &logo,
            DrawColor::White,
        )?;
    }
    renderer.present()?;
    Ok(())
}
