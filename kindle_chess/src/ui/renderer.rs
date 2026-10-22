use crate::ui::events::RectangleExt;
use fontdue::{Font, FontSettings};
use image::{ImageBuffer, Luma, Rgba, imageops};
use log::info;
use std::collections::HashMap;
use std::sync::Arc as StdArc;
use x11rb::COPY_DEPTH_FROM_PARENT;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, *};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/AdwaitaSans-Regular.ttf");

pub struct Renderer {
    conn: StdArc<x11rb::rust_connection::RustConnection>,
    screen_num: usize,
    window: Window,
    gcs: HashMap<DrawColor, Gcontext>,
    font: Font,
    dirty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrawColor {
    Black,
    White,
    Gray,
    DarkGray,
    LightGray,
}

impl Renderer {
    pub fn new()
    -> Result<(Self, StdArc<x11rb::rust_connection::RustConnection>), Box<dyn std::error::Error>>
    {
        // Connect to X11
        let (conn, screen_num) = x11rb::connect(None)?;

        // Wrap connection in Arc for sharing
        let conn = StdArc::new(conn);

        info!("Connected to X11, screen number: {}", screen_num);

        let screen = &conn.setup().roots[screen_num];

        // Create window
        let window = conn.generate_id()?;

        let win_aux = CreateWindowAux::new().background_pixel(screen.white_pixel);

        // Build event mask - add pointer events only for debug builds
        let event_mask = EventMask::EXPOSURE
            | EventMask::BUTTON_PRESS
            | EventMask::BUTTON_RELEASE
            | EventMask::KEY_PRESS
            | EventMask::STRUCTURE_NOTIFY;

        let event_mask = if cfg!(debug_assertions) {
            event_mask
                | EventMask::POINTER_MOTION
                | EventMask::ENTER_WINDOW
                | EventMask::LEAVE_WINDOW
        } else {
            event_mask
        };

        let win_aux = win_aux.event_mask(event_mask);

        // For release builds (Kindle), use override_redirect to avoid window manager interference
        // For debug builds, we want the window manager to handle the cursor
        let win_aux = if !cfg!(debug_assertions) {
            win_aux.override_redirect(1)
        } else {
            win_aux
        };

        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            window,
            screen.root,
            0,
            0,
            1072,
            1448,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &win_aux,
        )?;

        // Create GCs
        let mut gcs = HashMap::new();

        for (color, pixel_value) in [
            (DrawColor::Black, 0),
            (DrawColor::White, 255),
            (DrawColor::Gray, 128),
            (DrawColor::DarkGray, 64),
            (DrawColor::LightGray, 192),
        ] {
            let gc = conn.generate_id()?;
            conn.create_gc(
                gc,
                window,
                &CreateGCAux::new()
                    .foreground(pixel_value)
                    .background(if pixel_value > 128 { 0 } else { 255 }),
            )?;
            gcs.insert(color, gc);
        }

        // Map window
        conn.map_window(window)?;
        conn.configure_window(
            window,
            &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
        )?;

        // Request input focus for the window (debug builds only for cursor visibility)
        if cfg!(debug_assertions) {
            let _ = conn.set_input_focus(
                xproto::InputFocus::POINTER_ROOT,
                window,
                xproto::Time::CURRENT_TIME,
            );
        }

        conn.flush()?;

        let font = Font::from_bytes(FONT_BYTES, FontSettings::default())
            .map_err(|e| format!("failed to parse embedded font: {}", e))?;

        let renderer = Self {
            conn: conn.clone(), // Clone the Arc
            screen_num,
            window,
            gcs,
            font,
            dirty: true,
        };

        Ok((renderer, conn)) // Return both renderer and connection Arc
    }

    pub fn draw_rectangle(
        &mut self,
        rect: Rectangle,
        color: DrawColor,
        filled: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let gc = self.gcs[&color];

        if filled {
            self.conn.poly_fill_rectangle(self.window, gc, &[rect])?;
        } else {
            self.conn.poly_rectangle(self.window, gc, &[rect])?;
        }

        self.dirty = true;
        Ok(())
    }

    pub fn draw_circle(
        &mut self,
        center_x: i16,
        center_y: i16,
        radius: u16,
        color: DrawColor,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let gc = self.gcs[&color];

        // Use the X11 Arc type explicitly
        self.conn.poly_arc(
            self.window,
            gc,
            &[xproto::Arc {
                // Explicitly use xproto::Arc
                x: center_x - radius as i16,
                y: center_y - radius as i16,
                width: radius * 2,
                height: radius * 2,
                angle1: 0,
                angle2: 360 * 64,
            }],
        )?;

        self.dirty = true;
        Ok(())
    }

    /// Draw a line segment at thickness `width`. `width` of 0 or 1 uses the
    /// server's default thin-line algorithm (1 px). Anything ≥ 2 temporarily
    /// changes the GC's `line_width` to draw the segment, then resets it back
    /// to 0 — leaking a non-zero width would make subsequent unfilled
    /// rectangles draw thick borders too.
    pub fn draw_line(
        &mut self,
        x1: i16,
        y1: i16,
        x2: i16,
        y2: i16,
        color: DrawColor,
        width: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let gc = self.gcs[&color];

        if width >= 2 {
            self.conn
                .change_gc(gc, &ChangeGCAux::new().line_width(width as u32))?;
        }
        self.conn
            .poly_segment(self.window, gc, &[Segment { x1, y1, x2, y2 }])?;
        if width >= 2 {
            self.conn.change_gc(gc, &ChangeGCAux::new().line_width(0))?;
        }

        self.dirty = true;
        Ok(())
    }

    pub fn draw_image(
        &mut self,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        img: &ImageBuffer<Luma<u8>, Vec<u8>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use image::imageops::FilterType;
        use x11rb::protocol::xproto::ImageFormat;

        let scaled = imageops::resize(img, width as u32, height as u32, FilterType::Nearest);

        let screen = &self.conn.setup().roots[self.screen_num];
        let depth = screen.root_depth;

        // Each pixel must match the drawable depth:
        //   8-bit  → 1 byte per pixel
        //   16-bit → 2 bytes per pixel
        //   24/32  → 4 bytes per pixel (BGRX, scanlines still padded to 4 bytes)
        let bytes_per_pixel: usize = match depth {
            8 => 1,
            16 => 2,
            _ => 4, // 24 or 32-bit — the common dev-machine case
        };

        let row_bytes = width as usize * bytes_per_pixel;
        let width_padded = row_bytes.div_ceil(4) * 4;
        let mut data: Vec<u8> = Vec::with_capacity(width_padded * height as usize);

        for row in scaled.rows() {
            let mut row_data: Vec<u8> = Vec::with_capacity(row_bytes);
            for px in row {
                let v = px[0];
                match bytes_per_pixel {
                    1 => row_data.push(v),
                    2 => {
                        row_data.push(v);
                        row_data.push(v);
                    }
                    _ => {
                        row_data.push(v);
                        row_data.push(v);
                        row_data.push(v);
                        row_data.push(0);
                    }
                }
            }
            // Pad scanline to 4-byte boundary
            row_data.resize(width_padded, 0);
            data.extend_from_slice(&row_data);
        }

        self.conn.put_image(
            ImageFormat::Z_PIXMAP,
            self.window,
            self.gcs[&DrawColor::Black],
            width,
            height,
            x,
            y,
            0,     // left_pad
            depth, // use actual drawable depth, not hardcoded 8
            &data,
        )?;

        self.dirty = true;
        Ok(())
    }

    /// Composite an RGBA image onto a solid `background` color and dispatch
    /// via the regular grayscale `draw_image` path. Pixels are converted to
    /// luminance (BT.601 weights), then alpha-blended over the background so
    /// transparent PNG areas pick up the underlying square color. Used for
    /// piece sprites — they're 8-bit RGBA with a transparent background, and
    /// the e-ink panel only renders luma anyway.
    pub fn draw_image_alpha(
        &mut self,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        img: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        background: DrawColor,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use image::imageops::FilterType;

        let bg = color_to_luma(background) as u32;
        let scaled = imageops::resize(img, width as u32, height as u32, FilterType::Triangle);

        let mut composited: ImageBuffer<Luma<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(width as u32, height as u32, Luma([bg as u8]));

        for (px, py, pixel) in scaled.enumerate_pixels() {
            let [r, g, b, a] = pixel.0;
            if a == 0 {
                continue;
            }
            let luma = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
            let alpha = a as u32;
            let blended = (luma * alpha + bg * (255 - alpha)) / 255;
            composited.put_pixel(px, py, Luma([blended as u8]));
        }

        self.draw_image(x, y, width, height, &composited)
    }

    /// Returns the bounding-box size of `text` rendered at `size_px`, matching
    /// what `draw_text` would produce. Width is the sum of glyph advances,
    /// height is `ascent + descent` (so descender room is reserved even when
    /// the string has none — slight optical mis-centring is the trade-off).
    pub fn measure_text(&self, text: &str, size_px: f32) -> (u32, u32) {
        match self.line_geometry(text, size_px) {
            Some((w, ascent, descent)) => (w, (ascent + descent).max(1) as u32),
            None => (0, 0),
        }
    }

    fn line_geometry(&self, text: &str, size_px: f32) -> Option<(u32, i32, i32)> {
        if text.is_empty() {
            return None;
        }
        let line = self.font.horizontal_line_metrics(size_px)?;
        let ascent = line.ascent.ceil() as i32;
        let descent = (-line.descent).ceil() as i32; // fontdue's descent is negative
        let advance: f32 = text
            .chars()
            .map(|c| self.font.metrics(c, size_px).advance_width)
            .sum();
        Some((advance.ceil() as u32, ascent, descent))
    }

    /// Draws `text` with its top-left at (x, y) on a white background.
    /// `size_px` is the cap height in pixels (Adwaita Sans cap ≈ 0.7 × size).
    /// Glyph coverage is alpha-blended onto a transient grayscale buffer
    /// (initialised to white) which is then put_image'd via `draw_image`.
    pub fn draw_text(
        &mut self,
        x: i16,
        y: i16,
        text: &str,
        size_px: f32,
        color: DrawColor,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some((buf_width, ascent, descent)) = self.line_geometry(text, size_px) else {
            return Ok(());
        };
        let buf_height = (ascent + descent).max(1) as u32;
        if buf_width == 0 {
            return Ok(());
        }

        let fg = color_to_luma(color) as i32;
        let mut buffer: ImageBuffer<Luma<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(buf_width, buf_height, Luma([255]));

        let mut pen_x: f32 = 0.0;
        for c in text.chars() {
            let (m, bitmap) = self.font.rasterize(c, size_px);
            if m.width > 0 && m.height > 0 {
                let glyph_left = (pen_x + m.xmin as f32).round() as i32;
                let glyph_top = ascent - m.height as i32 - m.ymin;
                for row in 0..m.height {
                    for col in 0..m.width {
                        let cov = bitmap[row * m.width + col] as i32;
                        if cov == 0 {
                            continue;
                        }
                        let dx = glyph_left + col as i32;
                        let dy = glyph_top + row as i32;
                        if dx < 0 || dy < 0 || dx >= buf_width as i32 || dy >= buf_height as i32 {
                            continue;
                        }
                        let bg = buffer.get_pixel(dx as u32, dy as u32)[0] as i32;
                        let blended = bg + cov * (fg - bg) / 255;
                        buffer.put_pixel(dx as u32, dy as u32, Luma([blended.clamp(0, 255) as u8]));
                    }
                }
            }
            pen_x += m.advance_width;
        }

        self.draw_image(x, y, buf_width as u16, buf_height as u16, &buffer)
    }

    pub fn clear(&mut self, color: DrawColor) -> Result<(), Box<dyn std::error::Error>> {
        self.draw_rectangle(Rectangle::new(0, 0, 1072, 1448), color, true)
    }

    pub fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.dirty {
            self.conn.flush()?;
            self.dirty = false;
        }
        Ok(())
    }

    pub fn window(&self) -> Window {
        self.window
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self.conn.destroy_window(self.window);
        let _ = self.conn.flush();
    }
}

fn color_to_luma(c: DrawColor) -> u8 {
    match c {
        DrawColor::Black => 0,
        DrawColor::DarkGray => 64,
        DrawColor::Gray => 128,
        DrawColor::LightGray => 192,
        DrawColor::White => 255,
    }
}
