use std::sync::mpsc;

use log::info;

use crate::models::ui::Display;

impl Display {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let (event_tx, event_rx) = mpsc::channel();
        let (renderer, conn) = crate::ui::renderer::Renderer::new()?;
        info!("Starting Display isntance");

        Ok(Self {
            renderer,
            conn,
            event_tx,
            event_rx,
            tap_times: Vec::new(),
            last_tap_pos: None,
        })
    }

    pub fn chrome_mode(&self) -> bool {
        self.renderer.chrome_mode()
    }

    /// Switch between fullscreen and status-bar-chrome mode. Chrome mode
    /// re-enables pillow so AWM draws the PC:T status bar over the window;
    /// fullscreen disables pillow again and clears the panel for our redraw.
    pub fn set_chrome_mode(&mut self, chrome: bool) -> Result<(), Box<dyn std::error::Error>> {
        if chrome == self.renderer.chrome_mode() {
            return Ok(());
        }
        if chrome {
            crate::system::pillow::enable_pillow();
        }
        self.renderer.set_chrome_mode(chrome)?;
        if !chrome {
            crate::system::pillow::disable_pillow();
            crate::system::pillow::eips_clear();
        }
        Ok(())
    }
}
