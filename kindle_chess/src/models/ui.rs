use std::{
    error::Error,
    sync::{
        Arc,
        mpsc::{Receiver, Sender},
    },
    time::Instant,
};

use crate::ui::{events::AppEvent, renderer::Renderer};

// ─── Display ──────────────────────────────────────────────────────────────────
// The single long-lived X11 resource. Created once in main() and borrowed by
// every Screen implementation for drawing and event routing.

pub struct Display {
    pub renderer: Renderer,
    pub conn: Arc<x11rb::rust_connection::RustConnection>,
    pub event_tx: Sender<AppEvent>,
    pub event_rx: Receiver<AppEvent>,

    // Triple-tap detection lives here because it is global (works on any screen)
    pub tap_times: Vec<Instant>,
    pub last_tap_pos: Option<(i16, i16)>,
}

// ─── Screen ───────────────────────────────────────────────────────────────────
// A Screen owns only widgets and screen-local state.
// It borrows Display for drawing and returns a Transition to drive navigation.

pub trait Screen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn Error>>;
    fn handle_event(
        &mut self,
        event: AppEvent,
        display: &mut Display,
    ) -> Result<Transition, Box<dyn Error>>;

    /// Called when this screen becomes the top of the stack again because a
    /// screen above it was popped. While it was covered, the framebuffer was
    /// drawn over by the screen on top — so any widget that does partial/diff
    /// rendering (the board) must be invalidated here, and screens showing
    /// fetched data (ongoing games) can refresh. Default: no-op.
    fn on_reveal(&mut self) {}
}

// ─── Transition ───────────────────────────────────────────────────────────────

pub enum Transition {
    Stay,                  // keep current screen, no redraw needed
    Redraw,                // keep current screen, request a redraw
    Push(Box<dyn Screen>), // navigate forward to a new screen
    Pop,                   // return to the previous screen
    Quit,                  // exit the application
}
