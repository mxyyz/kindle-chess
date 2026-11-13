use std::sync::{Arc, RwLock, mpsc::Sender};

use log::{debug, error, info, warn};

use crate::i18n::{self};

use super::*;

use crate::ui::layout;
use crate::{
    models::{
        board_api::{RematchContext, Turn},
        chess::ChessApp,
        ui::{Display, Screen, Transition},
    },
    ui::{
        events::{AppEvent, Rectangle, RectangleExt},
        widgets::{BoardWidget, Button, SidebarWidget},
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
    // e06s04: set when the stream reports a terminal position; the rematch
    // button is only shown when a RematchContext is attached (games opened
    // from Play with a known challenge) and the game is over.
    pub game_over: bool,
    pub rematch: Option<RematchContext>,
    pub rematch_button: Button,
}

impl ChessGameScreen {
    pub fn new(app: ChessApp, config: Arc<RwLock<AppConfig>>) -> Self {
        // Seed the sidebar with the initial turn from `attach_game` so the
        // sidebar shows "Your turn" / "Waiting…" immediately, before the
        // game-state stream catches up.
        let loc = crate::i18n::locale_of(&config);
        let mut sidebar = SidebarWidget::new(Rectangle::new(
            0,
            layout::SIDEBAR_Y,
            layout::SIDEBAR_W,
            layout::SIDEBAR_H,
        ));
        if let Some(turn) = app.turn() {
            sidebar.set_turn(turn.clone(), &loc);
        }
        // e07s12: read board_labels before config is moved into the struct.
        let board_labels = config.read().map(|c| c.board_labels).unwrap_or(true);
        Self {
            app,
            config,
            board: {
                let mut b =
                    BoardWidget::new(Rectangle::new(0, 0, layout::BOARD_SIZE, layout::BOARD_SIZE));
                b.set_show_labels(board_labels);
                b
            },
            sidebar,
            stream_started: false,
            stream_cancel: None,
            game_over: false,
            rematch: None,
            // Centered in the lower sidebar, clear of the corner icon buttons.
            rematch_button: Button::new(
                (layout::SCREEN_W - 600) / 2,
                layout::SIDEBAR_Y + layout::SIDEBAR_H as i16 - 128,
                600,
                96,
                "Rematch".into(),
                40.0,
                true,
            ),
        }
    }

    // ── e06s04: rematch ─────────────────────────────────────────────────────

    pub fn is_game_over(turn: &Turn) -> bool {
        matches!(turn, Turn::Over { .. })
    }

    pub fn set_rematch(&mut self, ctx: RematchContext) {
        self.rematch = Some(ctx);
    }

    pub fn mark_game_over(&mut self) {
        self.game_over = true;
    }

    pub fn rematch_button_visible(&self) -> bool {
        self.game_over && self.rematch.is_some()
    }

    /// Re-issue the original challenge. Result arrives as `ChallengeSent` /
    /// `AiGameStarted` / `ChallengeSendFailed`.
    fn kick_rematch(&mut self, display: &Display) {
        let Some(ctx) = self.rematch.clone() else {
            return;
        };
        let Some(api) = self.app.online_in_game_api() else {
            warn!("rematch with no online backend — ignored");
            return;
        };
        let tx = display.event_tx.clone();
        // Random is disabled until e11 — a stale rematch context must not send
        // an open challenge; surface "coming soon" as the failure reason.
        let random_soon = i18n::tr(&i18n::locale_of(&self.config), "play-kind-random-soon");
        tokio::spawn(async move {
            let result: Result<AppEvent, String> = match &ctx.kind {
                crate::models::board_api::OpponentKind::Friend(name) => api
                    .send_challenge(name, &ctx.mode, ctx.rated)
                    .await
                    .map(|_| PlayScreen::send_event(&ctx.kind, None))
                    .map_err(|e| e.to_string()),
                crate::models::board_api::OpponentKind::Computer(level) => api
                    .send_challenge_ai(*level, &ctx.mode)
                    .await
                    .map(|g| PlayScreen::send_event(&ctx.kind, Some(g.id)))
                    .map_err(|e| e.to_string()),
                crate::models::board_api::OpponentKind::Random => Err(random_soon.clone()),
            };
            match result {
                Ok(ev) => {
                    let _ = tx.send(ev);
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::ChallengeSendFailed(e));
                }
            }
        });
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
        // e06s04: rematch button once the game is over and a context exists.
        if self.rematch_button_visible() {
            self.rematch_button.label = i18n::tr(&i18n::locale_of(&self.config), "game-rematch");
            self.rematch_button.draw(&mut display.renderer)?;
        }
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
                let over = Self::is_game_over(&turn);
                self.sidebar
                    .set_turn(turn, &crate::i18n::locale_of(&self.config));
                if over {
                    self.mark_game_over();
                }
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
                    .set_turn(turn.clone(), &crate::i18n::locale_of(&self.config));
                if Self::is_game_over(&turn) {
                    self.mark_game_over();
                }
                Ok(Transition::Redraw)
            }

            // e06s04: rematch results arrive here while the game screen is top.
            AppEvent::ChallengeSent(opponent) => {
                info!("Rematch challenge sent to {opponent}");
                // Stop the old game's stream task before leaving (Push does
                // not cancel it); when the opponent accepts, InvitesScreen
                // attaches the new game id — attach_game re-scopes even from
                // this in-game backend (BUG-2026-11-11T145208).
                if let Some(old) = &self.stream_cancel {
                    old.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                Ok(Transition::Push(Box::new(InvitesScreen::new(
                    self.app.clone(),
                    self.config.clone(),
                ))))
            }

            AppEvent::AiGameStarted { game_id, my_turn } => {
                info!("Rematch AI game started: {game_id}");
                // BUG-2026-11-11T145208: the app is still scoped to the old
                // (finished) game here — attach_game re-scopes it to the new
                // id. Stop the old game's stream task too: Push does not
                // cancel it, and a mid-backoff task must not keep emitting
                // old-game events onto the new screen.
                if let Some(old) = &self.stream_cancel {
                    old.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                let game_app = rematch_app(&self.app, game_id, my_turn);
                let mut next = ChessGameScreen::new(game_app, self.config.clone());
                // keep the same context for a potential next rematch
                if let Some(ctx) = self.rematch.clone() {
                    next.set_rematch(ctx);
                }
                Ok(Transition::Push(Box::new(next)))
            }

            AppEvent::Touch(touch) => {
                if let Some(ev) = self.board.handle_touch(&touch) {
                    return self.handle_event(ev, display);
                }

                if let Some(ev) = self.sidebar.handle_touch(&touch) {
                    return self.handle_event(ev, display);
                }

                // e06s04: rematch button (only drawn when game over + context).
                if self.rematch_button_visible()
                    && touch.kind == crate::ui::events::TouchKind::Up
                    && self.rematch_button.rect.contains(touch.x, touch.y)
                {
                    self.kick_rematch(display);
                    return Ok(Transition::Redraw);
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

            // Chrome-mode toggle (formerly the top.png sidebar button; now
            // handled directly in SettingsScreen). Kept as a fallback handler.
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
                info!("Opening settings screen (game context)");
                Ok(Transition::Push(Box::new(
                    SettingsScreen::with_context_and_app(
                        self.config.clone(),
                        SettingsContext::Game,
                        Some(self.app.clone()),
                    ),
                )))
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
        // A sub-screen (settings) drew over the board + sidebar while we
        // were covered — force a full repaint instead of a stale partial diff.
        // e08: also re-read board_labels so toggles from the 2-tab settings
        // screen take effect regardless of which context opened it.
        if let Ok(c) = self.config.read() {
            self.board.set_show_labels(c.board_labels);
        }
        self.board.invalidate();
        self.sidebar.invalidate();
    }
}

/// BUG-2026-11-13T014911: when the screen is popped (or replaced), cancel its
/// game stream task so the old game's events stop flowing into the shared
/// channel — otherwise the next ChessGameScreen receives both games' events
/// and the board alternates between them.
impl Drop for ChessGameScreen {
    fn drop(&mut self) {
        if let Some(cancel) = &self.stream_cancel {
            cancel.store(true, std::sync::atomic::Ordering::SeqCst);
            info!("ChessGameScreen dropped — game stream cancelled");
        }
    }
}

/// Re-scope the app to a rematch game. `attach_game` works both from Idle and
/// from an already in-game backend (the latter is the rematch case —
/// BUG-2026-11-11T145208), so the pushed screen streams the NEW game id.
fn rematch_app(app: &ChessApp, game_id: String, my_turn: bool) -> ChessApp {
    app.clone().attach_game(game_id, my_turn)
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

#[cfg(test)]
mod rematch_tests {
    use super::*;
    use crate::models::board_api::{ChallengeMode, OpponentKind, RematchContext, Turn};
    use crate::ui::screens::test_config;

    fn screen() -> ChessGameScreen {
        ChessGameScreen::new(ChessApp::new_offline(), test_config())
    }

    #[test]
    fn e06s04_game_over_detected_from_turn_over() {
        assert!(ChessGameScreen::is_game_over(&Turn::Over { winner: None }));
        assert!(ChessGameScreen::is_game_over(&Turn::Over {
            winner: Some("white".into())
        }));
        assert!(!ChessGameScreen::is_game_over(&Turn::Playing));
        assert!(!ChessGameScreen::is_game_over(&Turn::Waiting));
    }

    #[test]
    fn e06s04_rematch_button_only_when_game_over_and_context_present() {
        let mut s = screen();
        assert!(!s.rematch_button_visible());
        s.set_rematch(RematchContext {
            kind: OpponentKind::Friend("Alice".into()),
            mode: ChallengeMode::RealTime,
            rated: true,
        });
        assert!(!s.rematch_button_visible(), "not game over yet");
        s.mark_game_over();
        assert!(s.rematch_button_visible());
        // Button must sit inside the sidebar area and clear of the corner icons.
        let r = &s.rematch_button.rect;
        assert!(r.x >= 0 && r.y >= layout::SIDEBAR_Y);
        assert!(r.x + r.width as i16 <= layout::SCREEN_W);
        assert!(r.y + r.height as i16 <= layout::SCREEN_H);
    }

    #[test]
    fn e06s04_rematch_context_preserved_verbatim() {
        let mut s = screen();
        let ctx = RematchContext {
            kind: OpponentKind::Computer(6),
            mode: ChallengeMode::Correspondence(7),
            rated: false,
        };
        s.set_rematch(ctx.clone());
        assert_eq!(s.rematch.as_ref(), Some(&ctx));
        assert_eq!(s.rematch.as_ref().unwrap().mode.days_value(), 7);
        assert_eq!(s.rematch.as_ref().unwrap().kind.level(), Some(6));
    }

    // BUG-2026-11-11T145208: a rematch fires while the app is scoped to the
    // OLD game — the pushed screen's app must end up scoped to the NEW game
    // id, or its stream loads the old finished game instead.
    #[test]
    fn rematch_app_rescopes_in_game_backend_to_new_game() {
        use crate::models::oauth::{LichessUser, TokenInfo};
        let token = TokenInfo {
            access_token: "test_token_12345".into(),
            token_type: "Bearer".into(),
            expires_in: Some(86400),
            scope: None,
        };
        let user = LichessUser {
            id: "testuser".into(),
            username: "TestUser".into(),
            perfs: None,
            created_at: None,
            disabled: None,
            tos_violation: None,
            profile: None,
            seen_at: None,
            patron: None,
            verified: None,
            play_time: None,
            title: None,
        };
        let app = ChessApp::new_online(token, user).unwrap();
        let app = app.attach_game("oldGame".into(), true);
        assert_eq!(app.online_in_game_api().unwrap().game_id(), "oldGame");

        let next_app = rematch_app(&app, "newGame".into(), true);
        assert_eq!(
            next_app.online_in_game_api().unwrap().game_id(),
            "newGame",
            "rematch screen must stream the NEW game, not the old one"
        );
        // The original app keeps the old scope (clone semantics).
        assert_eq!(app.online_in_game_api().unwrap().game_id(), "oldGame");
    }

    // BUG-2026-11-13T014911: dropping a ChessGameScreen must cancel its
    // stream task, otherwise the old game's events keep flowing into the
    // shared channel and the next screen's board alternates between games.
    #[test]
    fn drop_cancels_game_stream() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut s = screen();
            s.stream_started = true;
            s.stream_cancel = Some(cancel.clone());
            assert!(!cancel.load(Ordering::SeqCst));
        } // s dropped here
        assert!(
            cancel.load(Ordering::SeqCst),
            "Drop must set stream_cancel so the old game's stream stops"
        );
    }

    #[test]
    fn drop_without_stream_is_noop() {
        // A screen that never started a stream (offline/demo) must drop cleanly.
        let s = screen();
        assert!(s.stream_cancel.is_none());
        drop(s); // must not panic
    }
}
