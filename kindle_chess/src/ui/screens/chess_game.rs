use std::sync::{Arc, RwLock, mpsc::Sender};

use log::{debug, error, info, warn};

use crate::i18n::{self};

use super::*;

use crate::{
    models::{
        chess::ChessApp,
        ui::{Display, Screen, Transition},
    },
    ui::{
        events::{AppEvent, Rectangle, RectangleExt},
        widgets::{BoardWidget, SidebarWidget},
    },
    utils::config::AppConfig,
};

// ─── ChessGameScreen ──────────────────────────────────────────────────────────

pub struct ChessGameScreen {
    pub app: ChessApp,
    pub config: Arc<RwLock<AppConfig>>,
    pub board: BoardWidget,
    pub sidebar: SidebarWidget,
    // First-render guard: kick the game-state stream task exactly once.
    pub stream_started: bool,
    // Cancel flag of the currently running stream task. Set on DeviceWake so
    // a stalled stream (device was locked) is superseded by a fresh one.
    pub stream_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl ChessGameScreen {
    pub fn new(app: ChessApp, config: Arc<RwLock<AppConfig>>) -> Self {
        // Seed the sidebar with the initial turn from `attach_game` so the
        // sidebar shows "Your turn" / "Waiting…" immediately, before the
        // game-state stream catches up.
        let loc = crate::i18n::locale_of(&config);
        let mut sidebar = SidebarWidget::new(Rectangle::new(0, 1072, 1072, 376));
        if let Some(turn) = app.turn() {
            sidebar.set_turn(turn.clone(), &loc);
        }
        Self {
            app,
            config,
            board: BoardWidget::new(Rectangle::new(0, 0, 1072, 1072)),
            sidebar,
            stream_started: false,
            stream_cancel: None,
        }
    }
}

// ─── ChessGameScreen ──────────────────────────────────────────────────────────

impl Screen for ChessGameScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        // First paint after Push: spawn the game-state stream task. It owns a
        // clone of BoardAPI<InGame>; everything we care about comes back as
        // GameFullReceived / TurnChanged events (see kick_game_stream).
        if !self.stream_started {
            self.stream_started = true;
            let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            self.stream_cancel = Some(cancel.clone());
            kick_game_stream(&self.app, display.event_tx.clone(), cancel);
        }

        self.sidebar.set_locale(&i18n::locale_of(&self.config));
        self.board.render(&mut display.renderer)?;
        self.sidebar.render(&mut display.renderer)?;
        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            AppEvent::GameFullReceived {
                white,
                black,
                player0_white,
                turn,
                board,
                last_move,
            } => {
                // The opponent is whichever colour the local player isn't.
                let opponent = if player0_white {
                    black.display_name()
                } else {
                    white.display_name()
                };
                self.sidebar.set_opponent(opponent);
                self.app
                    .apply_game_full(white, black, player0_white, turn.clone());
                // Orient the board so the local player's pieces are on the
                // bottom rank — flip when player0 is black. set_flipped is a
                // no-op when orientation is unchanged, so re-applying on
                // every GameFull is cheap.
                self.board.set_flipped(!player0_white);
                self.board.set_position(board);
                self.board.set_last_move(last_move);
                self.sidebar
                    .set_turn(turn, &crate::i18n::locale_of(&self.config));
                Ok(Transition::Redraw)
            }

            AppEvent::TurnChanged {
                turn,
                board,
                last_move,
            } => {
                self.app.apply_turn(turn.clone());
                self.board.set_position(board);
                self.board.set_last_move(last_move);
                self.sidebar
                    .set_turn(turn, &crate::i18n::locale_of(&self.config));
                Ok(Transition::Redraw)
            }

            AppEvent::Touch(touch) => {
                if let Some(ev) = self.board.handle_touch(&touch) {
                    return self.handle_event(ev, display);
                }

                if let Some(ev) = self.sidebar.handle_touch(&touch) {
                    return self.handle_event(ev, display);
                }

                Ok(Transition::Redraw)
            }

            AppEvent::MoveMade(chess_move) => {
                info!(
                    "Move: {} -> {}",
                    chess_move.from.to_algebraic(),
                    chess_move.to.to_algebraic(),
                );
                let Some(uci) = self.board.move_to_uci(chess_move) else {
                    warn!("MoveMade before board position loaded — dropping move");
                    return Ok(Transition::Redraw);
                };
                if let Some(api) = self.app.online_in_game_api() {
                    // On a rejected move (illegal — e.g. doesn't resolve a
                    // check) post MoveRejected so the sidebar can flag it.
                    let tx = display.event_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = api.move_piece(&uci).await {
                            warn!("move_piece({}) failed: {}", uci, e);
                            let _ = tx.send(AppEvent::MoveRejected);
                        }
                    });
                } else {
                    warn!("MoveMade with no online in-game backend — ignored");
                }
                Ok(Transition::Redraw)
            }

            AppEvent::MoveRejected => {
                info!("Move rejected by server — flagging sidebar");
                self.sidebar.set_move_rejected();
                Ok(Transition::Redraw)
            }

            AppEvent::SquareSelected(square) => {
                info!("Selected square: {}", square.to_algebraic());
                Ok(Transition::Redraw)
            }

            // Sidebar top.png button: toggle the Kindle status bar (PC:T)
            // over the app window. Handled here (not in the app loop) because
            // the sidebar re-emits widget events via this screen-local
            // recursion — an app-loop handler would never see them.
            AppEvent::ToggleTopBar => {
                let new_mode = !display.chrome_mode();
                info!("ToggleTopBar — chrome mode -> {}", new_mode);
                if let Err(e) = display.set_chrome_mode(new_mode) {
                    error!("set_chrome_mode failed: {}", e);
                }
                Ok(Transition::Redraw)
            }

            AppEvent::ExitToMenu => {
                info!("Returning to home screen");
                Ok(Transition::Pop)
            }

            AppEvent::OpenGameActions => {
                info!("Opening game-actions screen");
                Ok(Transition::Push(Box::new(GameActionsScreen::new(
                    self.app.clone(),
                    self.config.clone(),
                ))))
            }

            AppEvent::Expose => {
                debug!("Expose event - redrawing");
                self.board.invalidate();
                self.sidebar.invalidate();
                Ok(Transition::Redraw)
            }

            AppEvent::WindowUnmapped => {
                warn!("Window unmapped!");
                Ok(Transition::Stay)
            }

            // Device woke from lock: the SSE stream stalls/dies while the
            // display is locked, so restart it — Lichess sends the full game
            // state (GameFull) on every new stream connection, which refetches
            // the most recent position. Cancel any stalled predecessor first.
            AppEvent::DeviceWake => {
                info!("Device wake — restarting game-state stream (refetch)");
                if let Some(old) = &self.stream_cancel {
                    old.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                self.stream_cancel = Some(cancel.clone());
                kick_game_stream(&self.app, display.event_tx.clone(), cancel);
                Ok(Transition::Redraw)
            }

            AppEvent::Quit => {
                info!("Quit requested");
                Ok(Transition::Quit)
            }

            _ => Ok(Transition::Stay),
        }
    }

    fn on_reveal(&mut self) {
        // A sub-screen (game actions) drew over the board + sidebar while we
        // were covered — force a full repaint instead of a stale partial diff.
        self.board.invalidate();
        self.sidebar.invalidate();
    }
}

// Spawns the game-state stream onto the tokio runtime. The task owns a fresh
// clone of `BoardAPI<InGame>`; mutations to the clone's `state` are local
// bookkeeping. Every state change the screen needs is sent back as an
// `AppEvent`. No-op when the screen wasn't pushed with an in-game backend
// (e.g. the Demo button path, which still uses an Idle ChessApp).
fn kick_game_stream(
    app: &ChessApp,
    tx: Sender<AppEvent>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let Some(mut api) = app.online_in_game_api() else {
        warn!("ChessGameScreen has no in-game backend — skipping stream");
        return;
    };
    tokio::spawn(async move {
        use crate::api::board::StreamEnd;
        use std::sync::atomic::Ordering;
        use std::time::Duration;

        // Resilient reconnect loop: the stream can die mid-session (e.g. the
        // transport drops while the display is locked) and the network may not
        // be back yet right at wake — so a failed/ended stream is re-established
        // with exponential backoff instead of giving up after one attempt.
        // Lichess sends the full GameFull state on every new connection, so a
        // reconnect also refetches the latest position.
        let mut backoff = Duration::from_secs(2);
        loop {
            if cancel.load(Ordering::SeqCst) {
                info!("game stream task cancelled");
                break;
            }
            match api.stream_game_event(tx.clone(), cancel.clone()).await {
                Ok(StreamEnd::Events { terminal: true }) => {
                    info!("game finished — game stream closed, not reconnecting");
                    break;
                }
                Ok(_) => {
                    // Stream delivered events then ended (e.g. died while the
                    // display was locked) — reconnect promptly.
                    backoff = Duration::from_secs(2);
                    info!("game stream ended — reconnecting");
                }
                Err(e) => {
                    warn!(
                        "Game-state stream errored: {} — reconnecting in {:?}",
                        e, backoff
                    );
                }
            }
            if cancel.load(Ordering::SeqCst) {
                info!("game stream task cancelled");
                break;
            }
            tokio::time::sleep(backoff).await;
            backoff = std::cmp::min(backoff * 2, Duration::from_secs(30));
        }
    });
}
