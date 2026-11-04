use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;

use crate::models::board_api::{BoardAPI, Idle, InGame};
use crate::models::board_local::BoardLocal;
use crate::ui::events::AppEvent;
use crate::ui::renderer::Renderer;
use crate::ui::widgets::{BoardWidget, SidebarWidget};

#[derive(Debug, Clone)]
pub struct ChessApp {
    pub backend: ChessBackend,
}

/// Tombstone — ChessUI is superseded by the Screen architecture in models/ui.rs.
/// ui/chess.rs still references this type and will be removed in a future cleanup.
#[allow(dead_code)]
pub struct ChessUI {
    pub renderer: Renderer,
    pub conn: Arc<x11rb::rust_connection::RustConnection>,
    pub event_tx: Sender<AppEvent>,
    pub event_rx: Receiver<AppEvent>,
    pub board: BoardWidget,
    pub sidebar: SidebarWidget,
    pub tap_times: Vec<Instant>,
    pub last_tap_pos: Option<(i16, i16)>,
}
// Two online variants reflect the API's compile-time state. HomeScreen and
// OngoingChessGamesScreen carry `OnlineIdle` (no game scoped). Picking an
// ongoing game transitions to `OnlineInGame`, which is what ChessGameScreen
// receives — that's also the only variant whose API surface exposes
// move_piece / resign / abort / stream_game_event.
#[derive(Debug, Clone)]
pub enum ChessBackend {
    Offline(BoardLocal),
    OnlineIdle(Box<BoardAPI<Idle>>),
    OnlineInGame(Box<BoardAPI<InGame>>),
}
