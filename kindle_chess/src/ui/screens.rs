use std::sync::Arc;
use std::time::{Duration, Instant};

use log::{debug, error, info, warn};

use crate::{
    api::{
        api::{get_home_puzzle, get_next_puzzle, post_puzzle_solution},
        github::{UpdateInfo, check_for_update},
        oauth::{authenticate, get_user_info, load_token},
        update::apply_update,
    },
    models::{
        board_api::{ChallengeMode, ChallengeSummary, PlayedBy},
        chess::ChessApp,
        oauth::TokenInfo,
        puzzle::{PuzzleParams, PuzzleSession, PuzzleStatus},
        ui::{
            ChessAuthScreen, ChessGameScreen, Display, FriendlistScreen, GameActionsScreen,
            HomeScreen, InvitesScreen, OngoingChessGamesScreen, PlayScreen, PuzzleScreen,
            PuzzleSettingsScreen, Screen, SettingsScreen, Transition, UpdateScreen, UpdateState,
        },
    },
    ui::{
        events::{AppEvent, RectangleExt, Square, TouchKind},
        renderer::{DrawColor, Renderer},
        events::Rectangle,
        widgets::Button,
    },
    version,
};
use std::sync::mpsc::Sender;

// ─── HomeScreen ───────────────────────────────────────────────────────────────

impl Screen for HomeScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        // First render: kick the silent auth bootstrap exactly once. The task
        // posts ChessReady (cached token still valid) or AuthFailed (need QR
        // flow) — both routed back through handle_event.
        if !self.auth_started {
            self.auth_started = true;
            kick_auth_bootstrap(display.event_tx.clone());
        }

        display.renderer.clear(DrawColor::White)?;

        // Logo, centred in the upper third. It was pre-scaled to its display
        // size in HomeScreen::new, so draw it at its own dimensions.
        if let Some(logo) = &self.logo {
            let dw = logo.width() as u16;
            let dh = logo.height() as u16;
            let upper_half = 1448 / 2;
            let lx = (1072 - dw as i16) / 2;
            let ly = (upper_half - dh as i16) / 2;
            display
                .renderer
                .draw_image_alpha(lx, ly, dw, dh, logo, DrawColor::White)?;
        }

        self.chess_button.draw(&mut display.renderer)?;
        self.puzzle_button.draw(&mut display.renderer)?;
        self.ongoing_games_button.draw(&mut display.renderer)?;
        self.settings_button.draw(&mut display.renderer)?;

        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        _display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            // Auth completed — either from our own bootstrap, or bubbled up
            // from a popped ChessAuthScreen after the QR flow finished.
            AppEvent::ChessReady(app) => {
                info!("ChessApp ready — buttons live");
                self.app = Some(*app);
                Ok(Transition::Redraw)
            }

            // Bootstrap couldn't authenticate silently. Hand control to the
            // QR-flow screen; on success it'll pop and re-emit ChessReady.
            AppEvent::AuthFailed(e) => {
                warn!("Silent auth failed ({}) — pushing ChessAuthScreen", e);
                Ok(Transition::Push(Box::new(ChessAuthScreen::new())))
            }

            AppEvent::Touch(touch) => {
                if touch.kind == TouchKind::Up {
                    // Settings is reachable regardless of auth state — an
                    // offline user still needs the update entry point.
                    if self.settings_button.rect.contains(touch.x, touch.y) {
                        info!("Settings button pressed");
                        return Ok(Transition::Push(Box::new(SettingsScreen::new())));
                    }

                    // Puzzle is also auth-independent: the daily puzzle needs
                    // no token. A token, if present, is forwarded so the
                    // settings screen can make tailored /puzzle/next requests.
                    if self.puzzle_button.rect.contains(touch.x, touch.y) {
                        info!("Puzzle button pressed");
                        let token = self.app.as_ref().and_then(|a| a.token());
                        return Ok(Transition::Push(Box::new(PuzzleScreen::new(
                            token,
                            self.config.clone(),
                        ))));
                    }

                    let Some(app) = self.app.clone() else {
                        info!("Button tap ignored — auth not yet complete");
                        return Ok(Transition::Stay);
                    };
                    if self.chess_button.rect.contains(touch.x, touch.y) {
                        info!("Play button pressed — pushing PlayScreen");
                        return Ok(Transition::Push(Box::new(PlayScreen::new(app))));
                    } else if self.ongoing_games_button.rect.contains(touch.x, touch.y) {
                        info!("Ongoing-games button pressed");
                        return Ok(Transition::Push(Box::new(OngoingChessGamesScreen::new(
                            app,
                        ))));
                    }
                }

                // A tap that hit no button changes nothing — staying put
                // avoids a full-screen clear+repaint (an e-ink flash).
                Ok(Transition::Stay)
            }

            AppEvent::Expose => {
                debug!("Expose event - redrawing");
                Ok(Transition::Redraw)
            }

            AppEvent::WindowUnmapped => {
                warn!("Window unmapped!");
                Ok(Transition::Stay)
            }

            AppEvent::Quit => {
                info!("Quit requested");
                Ok(Transition::Quit)
            }

            _ => Ok(Transition::Stay),
        }
    }
}

// Spawned from HomeScreen::render on first paint. Tries the cached token,
// validates it via get_user_info, and on success builds the ChessApp.
// Sends ChessReady on success, AuthFailed otherwise — never authenticate()s
// (that's ChessAuthScreen's job, since it owns the QR display).
fn kick_auth_bootstrap(tx: std::sync::mpsc::Sender<AppEvent>) {
    let maybe_token = load_token().map_err(|e| e.to_string());
    tokio::spawn(async move {
        match maybe_token {
            Ok(Some(token_info)) => {
                // Convert the non-Send Box<dyn Error> into a String before the
                // next .await so the future itself stays Send.
                let user_info = get_user_info(&token_info.access_token)
                    .await
                    .map_err(|e| e.to_string());
                match user_info {
                    Ok(user_info) => {
                        info!("Authenticated from cached token as: {}", user_info.username);
                        match ChessApp::new_online(token_info, user_info) {
                            Ok(app) => {
                                let _ = tx.send(AppEvent::ChessReady(Box::new(app)));
                            }
                            Err(e) => {
                                warn!("Failed to build online backend: {} — needs re-auth", e);
                                let _ = tx.send(AppEvent::AuthFailed(e.to_string()));
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Cached token rejected: {} — needs re-auth", e);
                        let _ = tx.send(AppEvent::AuthFailed(e));
                    }
                }
            }
            Ok(None) => {
                info!("No cached token on disk — needs auth");
                let _ = tx.send(AppEvent::AuthFailed("no token".into()));
            }
            Err(e) => {
                let _ = tx.send(AppEvent::AuthFailed(e));
            }
        }
    });
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
                self.sidebar.set_turn(turn);
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
                self.sidebar.set_turn(turn);
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

// ─── ChessAuthScreen ──────────────────────────────────────────────────────────

impl Screen for ChessAuthScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        // Kick the QR/PKCE flow exactly once. authenticate() will post
        // QrReady once the QR image is ready, and AuthSuccess once Lichess
        // redirects to the local callback.
        if !self.auth_started {
            self.auth_started = true;
            let tx = display.event_tx.clone();
            tokio::spawn(async move {
                match authenticate(tx.clone()).await {
                    Ok((token_info, user_info)) => {
                        info!("QR auth succeeded as: {}", user_info.username);
                        let _ = tx.send(AppEvent::AuthSuccess(token_info, user_info));
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::AuthFailed(e.to_string()));
                    }
                }
            });
        }

        display.renderer.clear(DrawColor::White)?;
        if let Some(ref img) = self.qr_image {
            display.renderer.draw_image(
                self.qr_code.x,
                self.qr_code.y,
                self.qr_code.width,
                self.qr_code.height,
                img,
            )?;
        } else {
            display
                .renderer
                .draw_rectangle(self.qr_code, DrawColor::LightGray, true)?;
        }
        display
            .renderer
            .draw_rectangle(self.auth_status, DrawColor::LightGray, true)?;
        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            AppEvent::AuthSuccess(token, user) => {
                // new_online is sync (no network I/O), but building the Lichess
                // client can still fail — route that back through the auth flow.
                match ChessApp::new_online(token, user) {
                    Ok(app) => {
                        let _ = display.event_tx.send(AppEvent::ChessReady(Box::new(app)));
                    }
                    Err(e) => {
                        warn!("Failed to build online backend: {}", e);
                        let _ = display.event_tx.send(AppEvent::AuthFailed(e.to_string()));
                    }
                }
                Ok(Transition::Stay)
            }

            // Re-emit so HomeScreen (top-of-stack after our Pop) captures the
            // app, then pop ourselves off.
            AppEvent::ChessReady(app) => {
                let _ = display.event_tx.send(AppEvent::ChessReady(app));
                Ok(Transition::Pop)
            }

            AppEvent::QrReady(img) => {
                self.qr_image = Some(img);
                Ok(Transition::Redraw)
            }

            AppEvent::AuthFailed(e) => {
                warn!("QR auth failed: {}", e);
                // TODO: surface error visually + offer retry. For now stay put.
                Ok(Transition::Stay)
            }

            _ => Ok(Transition::Stay),
        }
    }
}

// ─── OngoingChessGamesScreen ──────────────────────────────────────────────────────────

const GAMES_PER_PAGE: usize = 4;

impl OngoingChessGamesScreen {
    /// Spawn the ongoing-games fetch onto the tokio runtime. Idempotent w.r.t.
    /// `self.loading` — call freely from `render` (initial load) or a future
    /// reload-button handler. Result is delivered as `OngoingGamesLoaded` /
    /// `OngoingGamesFailed` over the shared event channel.
    fn kick_fetch(&mut self, display: &Display) {
        if self.loading {
            return;
        }
        let Some(api) = self.app.online_idle_api() else {
            self.error = Some("Offline backend has no ongoing games".into());
            return;
        };
        self.loading = true;
        let tx = display.event_tx.clone();
        tokio::spawn(async move {
            match api.get_ongoing_games(8).await {
                Ok(list) => {
                    let _ = tx.send(AppEvent::OngoingGamesLoaded(Arc::new(list)));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::OngoingGamesFailed(e.to_string()));
                }
            }
        });
    }

    fn page_count(&self) -> usize {
        match &self.games {
            Some(g) => g.now_playing.len().div_ceil(GAMES_PER_PAGE),
            None => 0,
        }
    }

    /// Bake labels for `index`'s page of games into the four chess-game buttons,
    /// and update `self.page_index`. No-op when `games` hasn't loaded yet — the
    /// `OngoingGamesLoaded` handler calls this again once data arrives. Slots
    /// past the available game count get a "-" so they're visibly inert; the
    /// touch handler also short-circuits taps on those slots.
    fn set_page(&mut self, index: usize) {
        self.page_index = index;
        let Some(games) = self.games.as_ref() else {
            return;
        };
        let buttons: [&mut Button; 4] = [
            &mut self.chessgame_button_0,
            &mut self.chessgame_button_1,
            &mut self.chessgame_button_2,
            &mut self.chessgame_button_3,
        ];
        let start = index * GAMES_PER_PAGE;
        for (k, btn) in buttons.into_iter().enumerate() {
            match games.now_playing.get(start + k) {
                Some(game) => {
                    let opp = match &game.opponent {
                        PlayedBy::User(player) => player.name.clone(),
                        PlayedBy::Ai(computer) => match computer.ai_level {
                            Some(level) => format!("AI lvl {}", level),
                            None => String::from("AI"),
                        },
                    };
                    let mut l = format!("VS {opp}");
                    if game.is_my_turn {
                        l = format!("> {l} <");
                    }
                    btn.label = l;
                }
                None => {
                    btn.label = "-".to_string();
                }
            }
        }
    }

    /// Resolve which game `chessgame_button_{k}` currently maps to, given the
    /// current page. Returns `None` for slots that are inert ("-"-labelled
    /// because the current page has fewer than 4 games left).
    fn game_at_slot(&self, k: usize) -> Option<&crate::models::board_api::GameData> {
        self.games
            .as_ref()?
            .now_playing
            .get(self.page_index * GAMES_PER_PAGE + k)
    }
}

impl Screen for OngoingChessGamesScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        use crate::ui::renderer::DrawColor;

        // First paint after Push: kick off the async fetch. Subsequent renders
        // (after data arrives or on reload) skip this branch.
        if self.games.is_none() && self.error.is_none() && !self.loading {
            self.kick_fetch(display);
        }

        display.renderer.clear(DrawColor::White)?;

        let size_px = 24.0;

        if let Some(err) = &self.error {
            let label = format!("Error: {}", err);
            let (tw, th) = display.renderer.measure_text(&label, size_px);
            let tx = (1072 - tw as i16) / 2;
            let ty = (1448 - th as i16) / 2;
            display
                .renderer
                .draw_text(tx, ty, &label, size_px, DrawColor::Black)?;
        } else if self.games.is_some() {
            // Labels were baked into the buttons by `set_page` (called from
            // OngoingGamesLoaded and from next/prev taps), so render is just
            // a draw pass.
            self.chessgame_button_0.draw(&mut display.renderer)?;
            self.chessgame_button_1.draw(&mut display.renderer)?;
            self.chessgame_button_2.draw(&mut display.renderer)?;
            self.chessgame_button_3.draw(&mut display.renderer)?;
            self.back_button.draw(&mut display.renderer)?;
            self.next_page_button.draw(&mut display.renderer)?;
            self.prev_page_button.draw(&mut display.renderer)?;
        } else {
            let label = "Loading…";
            let (tw, th) = display.renderer.measure_text(label, size_px);
            let tx = (1072 - tw as i16) / 2;
            let ty = (1448 - th as i16) / 2;
            display
                .renderer
                .draw_text(tx, ty, label, size_px, DrawColor::Black)?;
        }

        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        _display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            AppEvent::OngoingGamesLoaded(list) => {
                info!("Ongoing games loaded: {} entries", list.now_playing.len());
                self.games = Some(list);
                self.loading = false;
                // Bake labels for the current page now that we have data.
                self.set_page(self.page_index);
                Ok(Transition::Redraw)
            }

            AppEvent::OngoingGamesFailed(e) => {
                warn!("Ongoing games fetch failed: {}", e);
                self.error = Some(e);
                self.loading = false;
                Ok(Transition::Redraw)
            }

            AppEvent::Touch(touch) => {
                if touch.kind != TouchKind::Up {
                    return Ok(Transition::Stay);
                }

                if self.back_button.rect.contains(touch.x, touch.y) {
                    return Ok(Transition::Pop);
                }

                if self.next_page_button.rect.contains(touch.x, touch.y) {
                    let last = self.page_count().saturating_sub(1);
                    if self.page_index < last {
                        self.set_page(self.page_index + 1);
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }

                if self.prev_page_button.rect.contains(touch.x, touch.y) {
                    if self.page_index > 0 {
                        self.set_page(self.page_index - 1);
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }

                // Game-button taps. Resolve via `game_at_slot` so paginated
                // slots without backing data are inert.
                let game_buttons = [
                    &self.chessgame_button_0,
                    &self.chessgame_button_1,
                    &self.chessgame_button_2,
                    &self.chessgame_button_3,
                ];
                for (k, btn) in game_buttons.into_iter().enumerate() {
                    if !btn.rect.contains(touch.x, touch.y) {
                        continue;
                    }
                    let Some(game) = self.game_at_slot(k) else {
                        return Ok(Transition::Stay);
                    };
                    info!(
                        "Picked ongoing game (page {}, slot {}): {}",
                        self.page_index, k, game.game_id
                    );
                    let game_app = self
                        .app
                        .clone()
                        .attach_game(game.game_id.clone(), game.is_my_turn);
                    return Ok(Transition::Push(Box::new(ChessGameScreen::new(game_app))));
                }
                Ok(Transition::Stay)
            }

            AppEvent::Expose => Ok(Transition::Redraw),

            AppEvent::WindowUnmapped => {
                warn!("Window unmapped!");
                Ok(Transition::Stay)
            }

            AppEvent::Quit => Ok(Transition::Quit),

            _ => Ok(Transition::Stay),
        }
    }

    fn on_reveal(&mut self) {
        // Returning from a game (or any pushed screen): drop the cached list
        // so the next render re-fetches and the ongoing-games view is current.
        self.games = None;
        self.error = None;
    }
}

// ─── PlayScreen ───────────────────────────────────────────────────────────────

impl PlayScreen {
    fn mode(&self) -> ChallengeMode {
        match self.selected_mode {
            ChallengeMode::Correspondence(_) => {
                ChallengeMode::Correspondence(self.days_slider.value())
            }
            other => other,
        }
    }

    /// Spawn the challenge send. Result arrives as `ChallengeSent` /
    /// `ChallengeSendFailed` over the event channel.
    fn kick_send(&mut self, display: &Display) {
        let Some(opponent) = self.opponent.clone() else {
            self.status = Some("Select an opponent first".into());
            return;
        };
        let Some(api) = self.app.online_idle_api() else {
            self.status = Some("Offline backend cannot send challenges".into());
            return;
        };
        if self.sending {
            return;
        }
        self.sending = true;
        self.status = Some(format!("Sending challenge to {opponent}…"));
        let mode = self.mode();
        let tx = display.event_tx.clone();
        tokio::spawn(async move {
            match api.send_challenge(&opponent, &mode).await {
                Ok(_) => {
                    let _ = tx.send(AppEvent::ChallengeSent(opponent));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::ChallengeSendFailed(e.to_string()));
                }
            }
        });
    }

    /// The correspondence days-spinner is only shown in correspondence mode;
    /// in other modes its row stays blank (layout positions never move).
    fn spinner_visible(&self) -> bool {
        matches!(self.mode(), ChallengeMode::Correspondence(_))
    }

    /// The currently selected mode button, for drawing the selection marker.
    fn selected_mode_button(&self) -> &Button {
        match self.selected_mode {
            ChallengeMode::Unlimited => &self.mode_button_unlimited,
            ChallengeMode::RealTime => &self.mode_button_realtime,
            ChallengeMode::Correspondence(_) => &self.mode_button_correspondence,
        }
    }
}

/// Selected-file-style marker around `rect`: four 45° diagonal lines from
/// each corner pointing inward — same style as the chessboard's
/// selected-square highlight.
fn draw_selection_marker(
    renderer: &mut Renderer,
    rect: &Rectangle,
) -> Result<(), Box<dyn std::error::Error>> {
    let l = (rect.width.min(rect.height) / 8).max(12) as i16;
    let stroke = 4;
    let x = rect.x;
    let y = rect.y;
    let w = rect.width as i16;
    let h = rect.height as i16;
    let cuts = [
        (x, y, x + l, y + l),
        (x + w, y, x + w - l, y + l),
        (x + w, y + h, x + w - l, y + h - l),
        (x, y + h, x + l, y + h - l),
    ];
    for (x1, y1, x2, y2) in cuts {
        renderer.draw_line(x1, y1, x2, y2, DrawColor::Black, stroke)?;
    }
    Ok(())
}

impl Screen for PlayScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        use crate::ui::renderer::DrawColor;

        display.renderer.clear(DrawColor::White)?;

        let title = "Play vs friend";
        let (tw, _) = display.renderer.measure_text(title, 36.0);
        display.renderer.draw_text((1072 - tw as i16) / 2, 120, title, 36.0, DrawColor::Black)?;

        self.invites_button.draw(&mut display.renderer)?;
        self.mode_button_unlimited.draw(&mut display.renderer)?;
        self.mode_button_realtime.draw(&mut display.renderer)?;
        self.mode_button_correspondence.draw(&mut display.renderer)?;
        // Selection marker on the active mode button (selected-file style).
        draw_selection_marker(&mut display.renderer, &self.selected_mode_button().rect)?;

        // Slider row — only in correspondence mode; the row stays blank
        // otherwise so nothing below shifts.
        if self.spinner_visible() {
            self.days_slider.draw(&mut display.renderer)?;
        }

        self.opponent_button.label = self.opponent_label();
        self.opponent_button.draw(&mut display.renderer)?;
        self.send_button.draw(&mut display.renderer)?;

        if let Some(status) = &self.status {
            let (sw, sh) = display.renderer.measure_text(status, 24.0);
            display.renderer.draw_text(
                (1072 - sw as i16) / 2,
                1240 - sh as i16 / 2,
                status,
                24.0,
                DrawColor::Black,
            )?;
        }

        self.back_button.draw(&mut display.renderer)?;
        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            // Bubbled up from the popped FriendlistScreen.
            AppEvent::FriendSelected(name) => {
                info!("Opponent selected: {}", name);
                self.opponent = Some(name);
                self.status = None;
                Ok(Transition::Redraw)
            }

            AppEvent::ChallengeSent(opponent) => {
                self.sending = false;
                self.status = Some(format!("Challenge sent to {opponent}"));
                // Show the pending outgoing invite on the Invites screen.
                return Ok(Transition::Push(Box::new(InvitesScreen::new(
                    self.app.clone(),
                ))));
            }

            AppEvent::ChallengeSendFailed(e) => {
                warn!("Challenge send failed: {}", e);
                self.sending = false;
                self.status = Some(format!("Send failed: {e}"));
                Ok(Transition::Redraw)
            }

            AppEvent::Touch(touch) => {
                if touch.kind != TouchKind::Up {
                    return Ok(Transition::Stay);
                }

                if self.back_button.rect.contains(touch.x, touch.y) {
                    return Ok(Transition::Pop);
                }

                if self.mode_button_unlimited.rect.contains(touch.x, touch.y) {
                    self.selected_mode = ChallengeMode::Unlimited;
                    return Ok(Transition::Redraw);
                }
                if self.mode_button_realtime.rect.contains(touch.x, touch.y) {
                    self.selected_mode = ChallengeMode::RealTime;
                    return Ok(Transition::Redraw);
                }
                if self.mode_button_correspondence.rect.contains(touch.x, touch.y) {
                    self.selected_mode =
                        ChallengeMode::Correspondence(self.days_slider.value());
                    return Ok(Transition::Redraw);
                }

                // Slider tap sets the days value directly (single tap — safe
                // against the triple-tap-to-quit gesture).
                if self.days_slider.contains(touch.x, touch.y) {
                    let v = self.days_slider.tap_value(touch.x);
                    self.days_slider.set_value(v);
                    if matches!(self.selected_mode, ChallengeMode::Correspondence(_)) {
                        self.selected_mode = ChallengeMode::Correspondence(v);
                    }
                    return Ok(Transition::Redraw);
                }

                if self.opponent_button.rect.contains(touch.x, touch.y) {
                    info!("Opening friend list");
                    return Ok(Transition::Push(Box::new(FriendlistScreen::new(
                        self.app.clone(),
                    ))));
                }

                if self.send_button.rect.contains(touch.x, touch.y) {
                    self.kick_send(display);
                    return Ok(Transition::Redraw);
                }

                if self.invites_button.rect.contains(touch.x, touch.y) {
                    info!("Invites button pressed — pushing InvitesScreen");
                    return Ok(Transition::Push(Box::new(InvitesScreen::new(
                        self.app.clone(),
                    ))));
                }

                Ok(Transition::Stay)
            }

            AppEvent::Expose => Ok(Transition::Redraw),

            AppEvent::WindowUnmapped => {
                warn!("Window unmapped!");
                Ok(Transition::Stay)
            }

            AppEvent::Quit => Ok(Transition::Quit),

            _ => Ok(Transition::Stay),
        }
    }
}

// ─── InvitesScreen ────────────────────────────────────────────────────────────

/// How often the challenge list is re-polled while the screen is visible.
const INVITES_POLL_INTERVAL: Duration = Duration::from_secs(5);
/// Rows per section (incoming / outgoing) per page.
const INVITES_PER_PAGE: usize = 4;

/// First accepted challenge with a game id — that's a live game to join.
fn accepted_game(challenges: &[ChallengeSummary]) -> Option<&ChallengeSummary> {
    challenges.iter().find(|c| c.game_id().is_some())
}

/// Which challenge action an InvitesScreen row button triggers.
#[derive(Debug, Clone, Copy)]
enum Action {
    Accept,
    Decline,
    Cancel,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Action::Accept => "accept",
            Action::Decline => "decline",
            Action::Cancel => "cancel",
        }
    }
}

impl InvitesScreen {
    /// Spawn the challenge-list fetch. Idempotent w.r.t. `self.loading`.
    fn kick_fetch(&mut self, display: &Display) {
        if self.loading {
            return;
        }
        let Some(api) = self.app.online_idle_api() else {
            self.error = Some("Offline backend has no invites".into());
            return;
        };
        self.loading = true;
        let tx = display.event_tx.clone();
        tokio::spawn(async move {
            match api.list_challenges().await {
                Ok(list) => {
                    let _ = tx.send(AppEvent::ChallengesLoaded(Arc::new(list)));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::ChallengesFailed(e.to_string()));
                }
            }
        });
    }

    /// Spawn an accept/decline/cancel action, then re-fetch the list so the
    /// row disappears on success. The refreshed list arrives as
    /// `ChallengesLoaded`.
    fn kick_action(&mut self, display: &Display, id: String, action: Action) {
        let Some(api) = self.app.online_idle_api() else {
            self.status = Some("Offline backend".into());
            return;
        };
        self.status = Some(format!("Sending {}…", action.label()));
        let tx = display.event_tx.clone();
        tokio::spawn(async move {
            // Map the error to String immediately — `Box<dyn Error>` is not
            // Send, so it must not be held across the next `.await`.
            let res_msg = match action {
                Action::Accept => api.accept_challenge(&id).await.err(),
                Action::Decline => api.decline_challenge(&id).await.err(),
                Action::Cancel => api.cancel_challenge(&id).await.err(),
            }
            .map(|e| format!("{}: {e}", action.label()));
            match api.list_challenges().await {
                Ok(list) => {
                    let _ = tx.send(AppEvent::ChallengesLoaded(Arc::new(list)));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::ChallengesFailed(res_msg.unwrap_or_else(|| e.to_string())));
                }
            }
        });
    }

    fn incoming_page_count(&self) -> usize {
        match &self.challenges {
            Some(list) => list.iter().filter(|c| c.incoming).count().div_ceil(INVITES_PER_PAGE),
            None => 0,
        }
    }

    fn outgoing_page_count(&self) -> usize {
        match &self.challenges {
            Some(list) => list.iter().filter(|c| !c.incoming).count().div_ceil(INVITES_PER_PAGE),
            None => 0,
        }
    }

    /// Bake the current pages' challenges into the 4+4 rows. Slots past the
    /// list end keep a "-" label and are simply not drawn.
    fn set_rows(&mut self) {
        let Some(list) = self.challenges.as_ref() else {
            return;
        };
        let incoming: Vec<&ChallengeSummary> = list.iter().filter(|c| c.incoming).collect();
        let outgoing: Vec<&ChallengeSummary> = list.iter().filter(|c| !c.incoming).collect();
        let label = |src: &[&ChallengeSummary], page: usize, k: usize| {
            match src.get(page * INVITES_PER_PAGE + k) {
                Some(c) => format!("{} ({})", c.opponent, c.status),
                None => "-".to_string(),
            }
        };
        for k in 0..INVITES_PER_PAGE {
            self.incoming_name[k].label = label(&incoming, self.incoming_page, k);
            self.outgoing_name[k].label = label(&outgoing, self.outgoing_page, k);
        }
    }

    /// Whether row `k` of the section's current page has a backing challenge —
    /// empty rows are not rendered at all.
    fn incoming_visible(&self, k: usize) -> bool {
        self.incoming_at(k).is_some()
    }
    fn outgoing_visible(&self, k: usize) -> bool {
        self.outgoing_at(k).is_some()
    }

    fn incoming_at(&self, k: usize) -> Option<&ChallengeSummary> {
        self.challenges
            .as_ref()?
            .iter()
            .filter(|c| c.incoming)
            .nth(self.incoming_page * INVITES_PER_PAGE + k)
    }

    fn outgoing_at(&self, k: usize) -> Option<&ChallengeSummary> {
        self.challenges
            .as_ref()?
            .iter()
            .filter(|c| !c.incoming)
            .nth(self.outgoing_page * INVITES_PER_PAGE + k)
    }
}

impl Screen for InvitesScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        // Poll: initial fetch, then re-fetch every INVITES_POLL_INTERVAL.
        let stale = self
            .last_fetch
            .map(|t| t.elapsed() >= INVITES_POLL_INTERVAL)
            .unwrap_or(true);
        if !self.loading && self.error.is_none() && stale {
            self.kick_fetch(display);
        }

        display.renderer.clear(DrawColor::White)?;

        let title = "Invites";
        let (tw, _) = display.renderer.measure_text(title, 36.0);
        display.renderer.draw_text((1072 - tw as i16) / 2, 120, title, 36.0, DrawColor::Black)?;

        let mut label = |y: i16, text: &str| -> Result<(), Box<dyn std::error::Error>> {
            display.renderer.draw_text(136, y, text, 28.0, DrawColor::Black)?;
            Ok(())
        };
        label(200, "Incoming")?;
        label(680, "Outgoing")?;

        if let Some(err) = &self.error {
            let label = format!("Error: {}", err);
            let (w, h) = display.renderer.measure_text(&label, 24.0);
            display.renderer.draw_text(
                (1072 - w as i16) / 2,
                (1448 - h as i16) / 2,
                &label,
                24.0,
                DrawColor::Black,
            )?;
        } else if self.challenges.is_some() {
            // Dynamic rows: only slots backed by a challenge are drawn.
            for k in 0..INVITES_PER_PAGE {
                if self.incoming_visible(k) {
                    self.incoming_name[k].draw(&mut display.renderer)?;
                    self.incoming_accept[k].draw(&mut display.renderer)?;
                    self.incoming_decline[k].draw(&mut display.renderer)?;
                }
                if self.outgoing_visible(k) {
                    self.outgoing_name[k].draw(&mut display.renderer)?;
                    self.outgoing_cancel[k].draw(&mut display.renderer)?;
                }
            }
            self.incoming_prev.draw(&mut display.renderer)?;
            self.incoming_next.draw(&mut display.renderer)?;
            self.outgoing_prev.draw(&mut display.renderer)?;
            self.outgoing_next.draw(&mut display.renderer)?;
        } else {
            let label = "Loading…";
            let (w, h) = display.renderer.measure_text(label, 24.0);
            display.renderer.draw_text(
                (1072 - w as i16) / 2,
                (1448 - h as i16) / 2,
                label,
                24.0,
                DrawColor::Black,
            )?;
        }

        if let Some(status) = &self.status {
            let (w, h) = display.renderer.measure_text(status, 24.0);
            display.renderer.draw_text(
                (1072 - w as i16) / 2,
                1240 - h as i16 / 2,
                status,
                24.0,
                DrawColor::Black,
            )?;
        }

        self.back_button.draw(&mut display.renderer)?;
        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            AppEvent::ChallengesLoaded(list) => {
                info!("Challenges loaded: {} entries", list.len());
                self.last_fetch = Some(Instant::now());
                self.loading = false;
                self.error = None;
                // An accepted challenge means the game has started — join it.
                if let Some(c) = accepted_game(&list) {
                    if let Some(game_id) = c.game_id() {
                        let game_id = game_id.to_string();
                        info!("Challenge {} accepted — joining game {}", c.id, game_id);
                        let game_app = self.app.clone().attach_game(game_id, false);
                        return Ok(Transition::Push(Box::new(ChessGameScreen::new(game_app))));
                    }
                }
                self.challenges = Some(list);
                self.set_rows();
                Ok(Transition::Redraw)
            }

            AppEvent::ChallengesFailed(e) => {
                warn!("Challenge list fetch failed: {}", e);
                self.error = Some(e);
                self.loading = false;
                Ok(Transition::Redraw)
            }

            AppEvent::Touch(touch) => {
                if touch.kind != TouchKind::Up {
                    return Ok(Transition::Stay);
                }
                if self.back_button.rect.contains(touch.x, touch.y) {
                    return Ok(Transition::Pop);
                }

                // Per-section pagination: each section pages independently.
                if self.incoming_next.rect.contains(touch.x, touch.y) {
                    let last = self.incoming_page_count().saturating_sub(1);
                    if self.incoming_page < last {
                        self.incoming_page += 1;
                        self.set_rows();
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }
                if self.incoming_prev.rect.contains(touch.x, touch.y) {
                    if self.incoming_page > 0 {
                        self.incoming_page -= 1;
                        self.set_rows();
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }
                if self.outgoing_next.rect.contains(touch.x, touch.y) {
                    let last = self.outgoing_page_count().saturating_sub(1);
                    if self.outgoing_page < last {
                        self.outgoing_page += 1;
                        self.set_rows();
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }
                if self.outgoing_prev.rect.contains(touch.x, touch.y) {
                    if self.outgoing_page > 0 {
                        self.outgoing_page -= 1;
                        self.set_rows();
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }

                // Resolve the tapped row to (challenge id, action) first, so
                // the immutable button borrows end before `kick_action` borrows
                // `self` mutably. Empty slots are inert (not rendered, no hit).
                let mut pending: Option<(String, Action)> = None;
                for (k, btn) in self.incoming_accept.iter().enumerate() {
                    if btn.rect.contains(touch.x, touch.y) {
                        pending = self.incoming_at(k).map(|c| (c.id.clone(), Action::Accept));
                        break;
                    }
                }
                if pending.is_none() {
                    for (k, btn) in self.incoming_decline.iter().enumerate() {
                        if btn.rect.contains(touch.x, touch.y) {
                            pending = self.incoming_at(k).map(|c| (c.id.clone(), Action::Decline));
                            break;
                        }
                    }
                }
                if pending.is_none() {
                    for (k, btn) in self.outgoing_cancel.iter().enumerate() {
                        if btn.rect.contains(touch.x, touch.y) {
                            pending = self.outgoing_at(k).map(|c| (c.id.clone(), Action::Cancel));
                            break;
                        }
                    }
                }
                if let Some((id, action)) = pending {
                    self.kick_action(display, id, action);
                    return Ok(Transition::Redraw);
                }
                Ok(Transition::Stay)
            }

            AppEvent::Expose => Ok(Transition::Redraw),

            AppEvent::WindowUnmapped => {
                warn!("Window unmapped!");
                Ok(Transition::Stay)
            }

            AppEvent::Quit => Ok(Transition::Quit),

            _ => Ok(Transition::Stay),
        }
    }

    fn on_reveal(&mut self) {
        // Returning to this screen: force a refetch on next render.
        self.last_fetch = None;
        self.challenges = None;
        self.error = None;
    }
}

#[cfg(test)]
mod invites_tests {
    use super::*;
    use crate::models::board_api::ChallengeSummary;

    fn ch(id: &str, status: &str, url: &str, incoming: bool) -> ChallengeSummary {
        ChallengeSummary {
            id: id.into(),
            status: status.into(),
            opponent: "X".into(),
            incoming,
            url: url.into(),
        }
    }

    #[test]
    fn accepted_game_finds_only_accepted_with_url() {
        let list = vec![
            ch("a", "created", "https://lichess.org/", true),
            ch("b", "accepted", "https://lichess.org/gameB", false),
            ch("c", "accepted", "https://lichess.org/", true), // empty id segment
        ];
        let found = accepted_game(&list).unwrap();
        assert_eq!(found.id, "b");
        assert_eq!(found.game_id(), Some("gameB"));
    }

    #[test]
    fn no_accepted_means_no_game() {
        let list = vec![ch("a", "created", "https://lichess.org/", true)];
        assert!(accepted_game(&list).is_none());
    }

    #[test]
    fn rows_split_incoming_outgoing_and_hide_empty_slots() {
        let mut s = InvitesScreen::new(ChessApp::new_offline());
        s.challenges = Some(Arc::new(vec![
            ch("in1", "created", "u", true),
            ch("out1", "created", "u", false),
            ch("in2", "created", "u", true),
        ]));
        s.set_rows();
        assert_eq!(s.incoming_name[0].label, "X (created)");
        assert_eq!(s.incoming_name[1].label, "X (created)");
        assert!(s.incoming_visible(0));
        assert!(s.incoming_visible(1));
        assert!(!s.incoming_visible(2)); // not rendered
        assert!(s.outgoing_visible(0));
        assert!(!s.outgoing_visible(1));
        assert_eq!(s.incoming_at(0).unwrap().id, "in1");
        assert_eq!(s.incoming_at(2), None);
        assert_eq!(s.outgoing_at(0).unwrap().id, "out1");
        assert_eq!(s.incoming_page_count(), 1);
        assert_eq!(s.outgoing_page_count(), 1);
    }

    #[test]
    fn pagination_is_separate_per_section() {
        let mut s = InvitesScreen::new(ChessApp::new_offline());
        let mut list = Vec::new();
        for i in 0..5 {
            list.push(ch(&format!("in{i}"), "created", "u", true));
        }
        for i in 0..2 {
            list.push(ch(&format!("out{i}"), "created", "u", false));
        }
        s.challenges = Some(Arc::new(list));
        // Incoming has 5 -> 2 pages; outgoing has 2 -> 1 page.
        assert_eq!(s.incoming_page_count(), 2);
        assert_eq!(s.outgoing_page_count(), 1);
        // Paging incoming does not touch outgoing's page.
        s.incoming_page = 1;
        s.set_rows();
        assert!(s.incoming_visible(0)); // 5th invite
        assert!(!s.incoming_visible(1));
        assert_eq!(s.incoming_at(0).unwrap().id, "in4");
        assert_eq!(s.outgoing_page, 0);
        assert!(s.outgoing_visible(0));
        assert!(s.outgoing_visible(1));
        assert!(!s.outgoing_visible(2));
    }
}

#[cfg(test)]
mod play_screen_tests {
    use super::*;
    use crate::models::chess::ChessApp;

    #[test]
    fn opponent_label_reflects_selection() {
        let mut s = PlayScreen::new(ChessApp::new_offline());
        assert_eq!(s.opponent_label(), "Opponent: -");
        s.opponent = Some("Alice42".into());
        assert_eq!(s.opponent_label(), "Opponent: Alice42");
    }

    #[test]
    fn mode_tracks_slider_in_correspondence() {
        let mut s = PlayScreen::new(ChessApp::new_offline());
        s.selected_mode = ChallengeMode::Correspondence(s.days_slider.value());
        s.days_slider.set_value(4);
        assert_eq!(s.mode(), ChallengeMode::Correspondence(4));
        s.selected_mode = ChallengeMode::RealTime;
        assert_eq!(s.mode(), ChallengeMode::RealTime);
    }

    #[test]
    fn slider_only_visible_in_correspondence_mode() {
        let mut s = PlayScreen::new(ChessApp::new_offline());
        s.selected_mode = ChallengeMode::RealTime;
        assert!(!s.spinner_visible());
        s.selected_mode = ChallengeMode::Correspondence(3);
        assert!(s.spinner_visible());
        s.selected_mode = ChallengeMode::Unlimited;
        assert!(!s.spinner_visible());
    }

    #[test]
    fn selected_mode_button_tracks_selection() {
        let mut s = PlayScreen::new(ChessApp::new_offline());
        s.selected_mode = ChallengeMode::Unlimited;
        assert!(std::ptr::eq(s.selected_mode_button(), &s.mode_button_unlimited));
        s.selected_mode = ChallengeMode::Correspondence(3);
        assert!(std::ptr::eq(
            s.selected_mode_button(),
            &s.mode_button_correspondence
        ));
    }
}

// ─── FriendlistScreen ─────────────────────────────────────────────────────────

const FRIENDS_PER_PAGE: usize = 4;

impl FriendlistScreen {
    /// Spawn the friends fetch onto the tokio runtime. Idempotent w.r.t.
    /// `self.loading` — call from `render` (initial load) only. Result is
    /// delivered as `FriendsLoaded` / `FriendsFailed` over the event channel.
    fn kick_fetch(&mut self, display: &Display) {
        if self.loading {
            return;
        }
        let Some(api) = self.app.online_idle_api() else {
            self.error = Some("Offline backend has no friend list".into());
            return;
        };
        self.loading = true;
        let tx = display.event_tx.clone();
        tokio::spawn(async move {
            match api.get_following().await {
                Ok(names) => {
                    let _ = tx.send(AppEvent::FriendsLoaded(Arc::new(names)));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::FriendsFailed(e.to_string()));
                }
            }
        });
    }

    fn page_count(&self) -> usize {
        match &self.friends {
            Some(f) => f.len().div_ceil(FRIENDS_PER_PAGE),
            None => 0,
        }
    }

    /// Bake labels for `index`'s page of friends into the four buttons and
    /// update `self.page_index`. Slots past the list end get "-" (inert).
    fn set_page(&mut self, index: usize) {
        self.page_index = index;
        let Some(friends) = self.friends.as_ref() else {
            return;
        };
        let buttons: [&mut Button; 4] = [
            &mut self.friend_button_0,
            &mut self.friend_button_1,
            &mut self.friend_button_2,
            &mut self.friend_button_3,
        ];
        let start = index * FRIENDS_PER_PAGE;
        for (k, btn) in buttons.into_iter().enumerate() {
            match friends.get(start + k) {
                Some(name) => btn.label = name.clone(),
                None => btn.label = "-".to_string(),
            }
        }
    }

    /// Which friend `friend_button_{k}` currently maps to, given the page.
    /// `None` for inert "-" slots.
    fn friend_at_slot(&self, k: usize) -> Option<&String> {
        self.friends
            .as_ref()?
            .get(self.page_index * FRIENDS_PER_PAGE + k)
    }
}

impl Screen for FriendlistScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        use crate::ui::renderer::DrawColor;

        // First paint after Push: kick off the async fetch once.
        if self.friends.is_none() && self.error.is_none() && !self.loading {
            self.kick_fetch(display);
        }

        display.renderer.clear(DrawColor::White)?;

        // Title strip
        let title = "Choose opponent";
        let (tw, _) = display.renderer.measure_text(title, 36.0);
        display.renderer.draw_text(
            (1072 - tw as i16) / 2,
            120,
            title,
            36.0,
            DrawColor::Black,
        )?;

        let size_px = 24.0;
        if let Some(err) = &self.error {
            let label = format!("Error: {}", err);
            let (tw, th) = display.renderer.measure_text(&label, size_px);
            display.renderer.draw_text(
                (1072 - tw as i16) / 2,
                (1448 - th as i16) / 2,
                &label,
                size_px,
                DrawColor::Black,
            )?;
        } else if let Some(friends) = &self.friends {
            if friends.is_empty() {
                let label = "No friends yet — add friends on lichess.org";
                let (tw, th) = display.renderer.measure_text(label, size_px);
                display.renderer.draw_text(
                    (1072 - tw as i16) / 2,
                    (1448 - th as i16) / 2,
                    label,
                    size_px,
                    DrawColor::Black,
                )?;
            } else {
                self.friend_button_0.draw(&mut display.renderer)?;
                self.friend_button_1.draw(&mut display.renderer)?;
                self.friend_button_2.draw(&mut display.renderer)?;
                self.friend_button_3.draw(&mut display.renderer)?;
            }
            self.back_button.draw(&mut display.renderer)?;
            self.next_page_button.draw(&mut display.renderer)?;
            self.prev_page_button.draw(&mut display.renderer)?;
        } else {
            let label = "Loading…";
            let (tw, th) = display.renderer.measure_text(label, size_px);
            display.renderer.draw_text(
                (1072 - tw as i16) / 2,
                (1448 - th as i16) / 2,
                label,
                size_px,
                DrawColor::Black,
            )?;
            self.back_button.draw(&mut display.renderer)?;
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
            AppEvent::FriendsLoaded(names) => {
                info!("Friends loaded: {} entries", names.len());
                self.friends = Some(names);
                self.loading = false;
                self.set_page(self.page_index);
                Ok(Transition::Redraw)
            }

            AppEvent::FriendsFailed(e) => {
                warn!("Friends fetch failed: {}", e);
                self.error = Some(e);
                self.loading = false;
                Ok(Transition::Redraw)
            }

            AppEvent::Touch(touch) => {
                if touch.kind != TouchKind::Up {
                    return Ok(Transition::Stay);
                }

                if self.back_button.rect.contains(touch.x, touch.y) {
                    return Ok(Transition::Pop);
                }

                if self.next_page_button.rect.contains(touch.x, touch.y) {
                    let last = self.page_count().saturating_sub(1);
                    if self.page_index < last {
                        self.set_page(self.page_index + 1);
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }

                if self.prev_page_button.rect.contains(touch.x, touch.y) {
                    if self.page_index > 0 {
                        self.set_page(self.page_index - 1);
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }

                let friend_buttons = [
                    &self.friend_button_0,
                    &self.friend_button_1,
                    &self.friend_button_2,
                    &self.friend_button_3,
                ];
                for (k, btn) in friend_buttons.into_iter().enumerate() {
                    if !btn.rect.contains(touch.x, touch.y) {
                        continue;
                    }
                    let Some(name) = self.friend_at_slot(k).cloned() else {
                        return Ok(Transition::Stay);
                    };
                    info!("Friend selected: {}", name);
                    // Bubble the pick to the screen below before popping —
                    // it becomes top-of-stack and receives this event next.
                    let _ = display.event_tx.send(AppEvent::FriendSelected(name));
                    return Ok(Transition::Pop);
                }
                Ok(Transition::Stay)
            }

            AppEvent::Expose => Ok(Transition::Redraw),

            AppEvent::WindowUnmapped => {
                warn!("Window unmapped!");
                Ok(Transition::Stay)
            }

            AppEvent::Quit => Ok(Transition::Quit),

            _ => Ok(Transition::Stay),
        }
    }

    fn on_reveal(&mut self) {
        // Returning to this screen from a pushed one: refetch so the list is
        // current.
        self.friends = None;
        self.error = None;
    }
}

#[cfg(test)]
mod friendlist_tests {
    use super::*;
    use crate::models::chess::ChessApp;

    fn screen_with_friends(names: &[&str]) -> FriendlistScreen {
        let mut s = FriendlistScreen::new(ChessApp::new_offline());
        s.friends = Some(Arc::new(names.iter().map(|n| n.to_string()).collect()));
        s.set_page(0);
        s
    }

    #[test]
    fn pagination_pages_of_four() {
        let s = screen_with_friends(&["a", "b", "c", "d", "e"]);
        assert_eq!(s.page_count(), 2);
        assert_eq!(s.friend_button_0.label, "a");
        assert_eq!(s.friend_button_3.label, "d");
    }

    #[test]
    fn set_page_second_page_and_inert_slots() {
        let mut s = screen_with_friends(&["a", "b", "c", "d", "e"]);
        s.set_page(1);
        assert_eq!(s.friend_button_0.label, "e");
        assert_eq!(s.friend_button_1.label, "-");
        assert_eq!(s.friend_at_slot(0).map(|s| s.as_str()), Some("e"));
        assert_eq!(s.friend_at_slot(1), None);
    }

    #[test]
    fn empty_friend_list_has_one_empty_page() {
        let s = screen_with_friends(&[]);
        assert_eq!(s.page_count(), 0);
    }
}

// ─── SettingsScreen ───────────────────────────────────────────────────────────

impl Screen for SettingsScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        display.renderer.clear(DrawColor::White)?;

        // Title
        let title = "Settings";
        let title_size = 56.0;
        let (tw, _) = display.renderer.measure_text(title, title_size);
        display.renderer.draw_text(
            (1072 - tw as i16) / 2,
            120,
            title,
            title_size,
            DrawColor::Black,
        )?;

        // Version block
        let info_size = 28.0;
        let lines = [
            format!("Version:  {}", version::VERSION),
            format!("Commit:   {}", version::GIT_SHA),
            format!("Built:    {}", version::BUILD_TIMESTAMP),
        ];
        let mut y: i16 = 240;
        for line in &lines {
            display
                .renderer
                .draw_text(120, y, line, info_size, DrawColor::Black)?;
            y += 40;
        }

        self.check_update_button.draw(&mut display.renderer)?;
        self.back_button.draw(&mut display.renderer)?;
        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        _display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            AppEvent::Touch(touch) => {
                if touch.kind != TouchKind::Up {
                    return Ok(Transition::Stay);
                }
                if self.check_update_button.rect.contains(touch.x, touch.y) {
                    info!("Check-for-updates pressed");
                    return Ok(Transition::Push(Box::new(UpdateScreen::new())));
                }
                if self.back_button.rect.contains(touch.x, touch.y) {
                    return Ok(Transition::Pop);
                }
                Ok(Transition::Stay)
            }
            AppEvent::Expose => Ok(Transition::Redraw),
            AppEvent::WindowUnmapped => {
                warn!("Window unmapped!");
                Ok(Transition::Stay)
            }
            AppEvent::Quit => Ok(Transition::Quit),
            _ => Ok(Transition::Stay),
        }
    }
}

// ─── UpdateScreen ─────────────────────────────────────────────────────────────

impl Screen for UpdateScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        // Auto-kick the check on first paint.
        if !self.check_started {
            self.check_started = true;
            kick_update_check(display.event_tx.clone());
        }

        display.renderer.clear(DrawColor::White)?;

        // Title
        let title = "Update";
        let title_size = 56.0;
        let (tw, _) = display.renderer.measure_text(title, title_size);
        display.renderer.draw_text(
            (1072 - tw as i16) / 2,
            120,
            title,
            title_size,
            DrawColor::Black,
        )?;

        // State-driven body. The action button is only drawn (and only live in
        // the touch handler) when there is something meaningful to do.
        let (lines, action_label, action_visible) = match &self.state {
            UpdateState::Checking => (vec!["Checking for updates…".to_string()], "Apply", false),
            UpdateState::UpToDate => (
                vec![format!("You're up to date — v{}", version::VERSION)],
                "Apply",
                false,
            ),
            UpdateState::Available(info) => (
                vec![
                    format!("Update available: v{} → v{}", info.current, info.latest),
                    format!("({} bytes)", info.asset_size),
                    String::new(),
                    info.release_name.clone(),
                ],
                "Apply",
                true,
            ),
            UpdateState::Downloading => (
                vec!["Downloading and verifying…".to_string()],
                "Apply",
                false,
            ),
            UpdateState::Applied => (
                vec![
                    "Update applied.".to_string(),
                    "Quit and relaunch from the KUAL menu.".to_string(),
                ],
                "Quit",
                true,
            ),
            UpdateState::Failed(err) => (
                vec!["Update failed:".to_string(), err.clone()],
                "Apply",
                false,
            ),
        };

        let info_size = 28.0;
        let mut y: i16 = 280;
        for line in &lines {
            let (lw, _) = display.renderer.measure_text(line, info_size);
            let lx = (1072 - lw as i16) / 2;
            display
                .renderer
                .draw_text(lx, y, line, info_size, DrawColor::Black)?;
            y += 40;
        }

        if action_visible {
            self.action_button.label = action_label.to_string();
            self.action_button.draw(&mut display.renderer)?;
        }
        self.back_button.draw(&mut display.renderer)?;

        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            AppEvent::UpdateAvailable(info) => {
                info!("Update available: v{} → v{}", info.current, info.latest);
                self.state = UpdateState::Available(info);
                Ok(Transition::Redraw)
            }
            AppEvent::UpdateUpToDate => {
                self.state = UpdateState::UpToDate;
                Ok(Transition::Redraw)
            }
            AppEvent::UpdateCheckFailed(e) => {
                warn!("Update check failed: {}", e);
                self.state = UpdateState::Failed(format!("Check failed: {}", e));
                Ok(Transition::Redraw)
            }
            AppEvent::UpdateApplied => {
                info!("Update applied");
                self.state = UpdateState::Applied;
                Ok(Transition::Redraw)
            }
            AppEvent::UpdateApplyFailed(e) => {
                warn!("Update apply failed: {}", e);
                self.state = UpdateState::Failed(format!("Apply failed: {}", e));
                Ok(Transition::Redraw)
            }

            AppEvent::Touch(touch) => {
                if touch.kind != TouchKind::Up {
                    return Ok(Transition::Stay);
                }
                if self.back_button.rect.contains(touch.x, touch.y) {
                    return Ok(Transition::Pop);
                }
                if self.action_button.rect.contains(touch.x, touch.y) {
                    match &self.state {
                        UpdateState::Available(info) => {
                            let info = info.clone();
                            self.state = UpdateState::Downloading;
                            kick_update_apply(info, display.event_tx.clone());
                            return Ok(Transition::Redraw);
                        }
                        UpdateState::Applied => {
                            // KUAL will relaunch the binary the next time the
                            // user taps the menu entry — and exec(2) picks up
                            // the new file at the path we just renamed onto.
                            return Ok(Transition::Quit);
                        }
                        _ => {}
                    }
                }
                Ok(Transition::Stay)
            }

            AppEvent::Expose => Ok(Transition::Redraw),
            AppEvent::WindowUnmapped => {
                warn!("Window unmapped!");
                Ok(Transition::Stay)
            }
            AppEvent::Quit => Ok(Transition::Quit),
            _ => Ok(Transition::Stay),
        }
    }
}

fn kick_update_check(tx: Sender<AppEvent>) {
    tokio::spawn(async move {
        match check_for_update().await {
            Ok(Some(info)) => {
                let _ = tx.send(AppEvent::UpdateAvailable(info));
            }
            Ok(None) => {
                let _ = tx.send(AppEvent::UpdateUpToDate);
            }
            Err(e) => {
                let _ = tx.send(AppEvent::UpdateCheckFailed(e.to_string()));
            }
        }
    });
}

fn kick_update_apply(info: UpdateInfo, tx: Sender<AppEvent>) {
    tokio::spawn(async move {
        match apply_update(&info).await {
            Ok(()) => {
                let _ = tx.send(AppEvent::UpdateApplied);
            }
            Err(e) => {
                let _ = tx.send(AppEvent::UpdateApplyFailed(e.to_string()));
            }
        }
    });
}

// ─── PuzzleScreen ─────────────────────────────────────────────────────────────

impl PuzzleScreen {
    /// Fire-and-forget solve-post: when the session just reached `Solved` and
    /// the user is signed in, submit the result via POST /api/puzzle/batch so
    /// `/puzzle/next` advances and the solve lands in puzzle activity. Never
    /// blocks the UI — anonymous users and failed posts (e.g. 403 on tokens
    /// without `puzzle:write`) just log.
    fn maybe_post_solution(&self, solution: Option<String>) {
        let Some(id) = solution else { return };
        let Some(token) = self.token.clone() else {
            return;
        };
        let params = self.params;
        tokio::spawn(async move {
            match post_puzzle_solution(Some(token), params, &id, true).await {
                Ok(Some(rounds)) => info!("Posted puzzle solution to Lichess: {:?}", rounds),
                Ok(None) => {
                    info!("Puzzle solved locally; not posted (anonymous or request failed)")
                }
                Err(e) => warn!("post_puzzle_solution error: {}", e),
            }
        });
    }
}

impl Screen for PuzzleScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        // First paint after Push: fetch the opening puzzle — the daily one,
        // unless the signed-in account already played today's daily, in which
        // case a fresh puzzle is fetched instead. Result comes back as
        // PuzzleLoaded / PuzzleLoadFailed (see kick_home_puzzle).
        if !self.load_started {
            self.load_started = true;
            kick_home_puzzle(self.token.clone(), self.params, display.event_tx.clone());
        }

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
            // A puzzle (daily or next) arrived — rebuild the session and reset
            // the board to its starting position.
            AppEvent::PuzzleLoaded(puzzle) => match PuzzleSession::new(puzzle) {
                Ok(session) => {
                    info!(
                        "Puzzle loaded (rating {}, {} solution plies)",
                        session.rating,
                        session.solution.len()
                    );
                    // Orient the board so the solver's pieces are on the bottom
                    // rank — flip when the solver plays Black.
                    self.board.set_flipped(!session.solver_white);
                    self.board.set_position(session.position.clone());
                    self.board.set_last_move(session.last_move_mask);
                    // Drop any hint highlight carried over from a prior puzzle.
                    self.board.select_square(None);
                    // Currently shows no info on an unsolved puzzle so nothing gets spoiled
                    self.sidebar
                        .set_puzzle_info(session.rating, &session.themes, &session.status);
                    self.sidebar.set_status(&session.status);
                    self.session = Some(session);
                    Ok(Transition::Redraw)
                }
                Err(e) => {
                    warn!("Puzzle FEN rejected: {}", e);
                    self.sidebar.set_message("Could not load puzzle");
                    Ok(Transition::Redraw)
                }
            },

            AppEvent::PuzzleLoadFailed(e) => {
                warn!("Puzzle fetch failed: {}", e);
                self.sidebar.set_message("Could not load puzzle");
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

            // The solver attempted a move — validate it against the solution.
            AppEvent::MoveMade(chess_move) => {
                let Some(session) = self.session.as_mut() else {
                    return Ok(Transition::Redraw);
                };
                if !session.accepts_input() {
                    return Ok(Transition::Redraw);
                }
                let Some(uci) = self.board.move_to_uci(chess_move) else {
                    warn!("MoveMade before puzzle position loaded — dropping move");
                    return Ok(Transition::Redraw);
                };
                let status = session.try_move(&uci);
                info!("Puzzle move {} → {:?}", uci, status);
                // try_move applies only the solver's move on success and leaves
                // the position untouched on a wrong move, so re-syncing the
                // widget unconditionally is correct either way.
                self.board.set_position(session.position.clone());
                self.board.set_last_move(session.last_move_mask);
                self.sidebar.set_status(&status);
                // On a correct, non-final move the opponent's scripted reply is
                // held back ~1 s so the solver sees their own move land first
                // instead of both moves snapping in at once.
                if session.opponent_reply_pending() {
                    let tx = display.event_tx.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        let _ = tx.send(AppEvent::PuzzleOpponentMove);
                    });
                }
                self.sidebar
                    .set_puzzle_info(session.rating, &session.themes, &session.status);
                let to_post = session
                    .solution_to_post(self.token.is_some())
                    .map(str::to_string);
                self.maybe_post_solution(to_post);
                Ok(Transition::Redraw)
            }

            // The opponent's scripted reply, fired ~1 s after a correct move.
            AppEvent::PuzzleOpponentMove => {
                let Some(session) = self.session.as_mut() else {
                    return Ok(Transition::Stay);
                };
                // The puzzle may have been replaced (Next puzzle / settings)
                // during the delay — only apply the reply if still pending.
                if !session.opponent_reply_pending() {
                    return Ok(Transition::Stay);
                }
                let status = session.apply_opponent_reply();
                info!("Puzzle opponent reply → {:?}", status);
                self.board.set_position(session.position.clone());
                self.board.set_last_move(session.last_move_mask);
                self.sidebar.set_status(&status);
                self.sidebar
                    .set_puzzle_info(session.rating, &session.themes, &session.status);
                let to_post = session
                    .solution_to_post(self.token.is_some())
                    .map(str::to_string);
                self.maybe_post_solution(to_post);
                Ok(Transition::Redraw)
            }

            AppEvent::SquareSelected(square) => {
                info!("Puzzle square selected: {}", square.to_algebraic());
                Ok(Transition::Redraw)
            }

            // Hint button — mark the from-square of the next solution move so
            // the solver can see which piece to move.
            AppEvent::ShowPuzzleHint => {
                let from = self
                    .session
                    .as_ref()
                    .and_then(|s| s.next_expected_move())
                    .and_then(square_from_uci);
                match from {
                    Some(square) => {
                        info!("Puzzle hint: marking {}", square.to_algebraic());
                        self.board.select_square(Some(square));
                    }
                    None => info!("Puzzle hint requested with no move to hint"),
                }
                Ok(Transition::Redraw)
            }

            AppEvent::OpenPuzzleSettings => Ok(Transition::Push(Box::new(
                PuzzleSettingsScreen::new(self.config.clone()),
            ))),

            // The sidebar's "Next puzzle" button — refetch with current params.
            AppEvent::NextPuzzle => {
                self.sidebar.set_message("Loading…");
                // Clear the previous puzzle's rating/themes immediately at
                // press time (not when the new puzzle arrives) — an info change
                // triggers a full sidebar repaint, so no residue is left.
                self.sidebar.set_puzzle_info(0, &[], &PuzzleStatus::Solving);
                kick_next_puzzle(self.token.clone(), self.params, display.event_tx.clone());
                Ok(Transition::Redraw)
            }

            // Re-emitted by PuzzleSettingsScreen as it pops — adopt the chosen
            // params and fetch a puzzle with them.
            AppEvent::ApplyPuzzleParams(params) => {
                self.params = params;
                self.sidebar.set_message("Loading…");
                self.sidebar.set_puzzle_info(0, &[], &PuzzleStatus::Solving);
                kick_next_puzzle(self.token.clone(), self.params, display.event_tx.clone());
                Ok(Transition::Redraw)
            }

            AppEvent::ExitToMenu => Ok(Transition::Pop),

            AppEvent::Expose => {
                self.board.invalidate();
                self.sidebar.invalidate();
                Ok(Transition::Redraw)
            }

            AppEvent::WindowUnmapped => {
                warn!("Window unmapped!");
                Ok(Transition::Stay)
            }

            AppEvent::Quit => Ok(Transition::Quit),

            _ => Ok(Transition::Stay),
        }
    }

    fn on_reveal(&mut self) {
        // The settings screen drew over the board + sidebar while we were
        // covered — force a full repaint instead of a stale partial diff.
        self.board.invalidate();
        self.sidebar.invalidate();
    }
}

// Parse the from-square (the first two characters) of a UCI move into a board
// Square. Returns None if the string is too short or out of the a1–h8 range.
fn square_from_uci(uci: &str) -> Option<Square> {
    let b = uci.as_bytes();
    if b.len() < 2 {
        return None;
    }
    let file = b[0].checked_sub(b'a').filter(|&f| f < 8)?;
    let rank = b[1].checked_sub(b'1').filter(|&r| r < 8)?;
    Some(Square::new(file, rank))
}

// Fetch the home-screen puzzle: the daily puzzle, or a fresh one when the
// account has already played today's daily (see get_home_puzzle). Result
// delivered as PuzzleLoaded / PuzzleLoadFailed over the shared event channel.
fn kick_home_puzzle(token: Option<TokenInfo>, params: PuzzleParams, tx: Sender<AppEvent>) {
    tokio::spawn(async move {
        match get_home_puzzle(token, params).await {
            Ok(puzzle) => {
                let _ = tx.send(AppEvent::PuzzleLoaded(puzzle));
            }
            Err(e) => {
                let _ = tx.send(AppEvent::PuzzleLoadFailed(e.to_string()));
            }
        }
    });
}

// Fetch a /puzzle/next puzzle for `params`, optionally authenticated.
fn kick_next_puzzle(token: Option<TokenInfo>, params: PuzzleParams, tx: Sender<AppEvent>) {
    tokio::spawn(async move {
        match get_next_puzzle(token, params).await {
            Ok(puzzle) => {
                let _ = tx.send(AppEvent::PuzzleLoaded(puzzle));
            }
            Err(e) => {
                let _ = tx.send(AppEvent::PuzzleLoadFailed(e.to_string()));
            }
        }
    });
}

// ─── PuzzleSettingsScreen ─────────────────────────────────────────────────────

impl Screen for PuzzleSettingsScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        display.renderer.clear(DrawColor::White)?;

        let title = "Puzzle Settings";
        let title_size = 56.0;
        let (tw, _) = display.renderer.measure_text(title, title_size);
        display.renderer.draw_text(
            (1072 - tw as i16) / 2,
            150,
            title,
            title_size,
            DrawColor::Black,
        )?;

        self.difficulty_button.draw(&mut display.renderer)?;
        self.phase_button.draw(&mut display.renderer)?;
        self.color_button.draw(&mut display.renderer)?;
        self.fetch_button.draw(&mut display.renderer)?;
        self.back_button.draw(&mut display.renderer)?;

        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            AppEvent::Touch(touch) => {
                if touch.kind != TouchKind::Up {
                    return Ok(Transition::Stay);
                }

                // Tap-to-cycle option buttons.
                if self.difficulty_button.rect.contains(touch.x, touch.y) {
                    self.params.difficulty = self.params.difficulty.next();
                    self.difficulty_button.label =
                        format!("Difficulty: {}", self.params.difficulty.label());
                    return Ok(Transition::Redraw);
                }
                if self.phase_button.rect.contains(touch.x, touch.y) {
                    self.params.phase = self.params.phase.next();
                    self.phase_button.label = format!("Phase: {}", self.params.phase.label());
                    return Ok(Transition::Redraw);
                }
                if self.color_button.rect.contains(touch.x, touch.y) {
                    self.params.color = self.params.color.next();
                    self.color_button.label = format!("Color: {}", self.params.color.label());
                    return Ok(Transition::Redraw);
                }

                // Persist the confirmed choices in the shared config so they
                // outlive this screen and seed the next PuzzleScreen, then
                // hand them to PuzzleScreen (top of stack once we pop) to
                // drive the fetch.
                if self.fetch_button.rect.contains(touch.x, touch.y) {
                    info!("Fetching puzzle with {:?}", self.params);
                    match self.config.write() {
                        Ok(mut config) => config.puzzle = self.params,
                        Err(e) => {
                            warn!("Config store poisoned; puzzle params not persisted: {}", e)
                        }
                    }
                    let _ = display
                        .event_tx
                        .send(AppEvent::ApplyPuzzleParams(self.params));
                    return Ok(Transition::Pop);
                }
                if self.back_button.rect.contains(touch.x, touch.y) {
                    return Ok(Transition::Pop);
                }
                Ok(Transition::Stay)
            }

            AppEvent::Expose => Ok(Transition::Redraw),

            AppEvent::WindowUnmapped => {
                warn!("Window unmapped!");
                Ok(Transition::Stay)
            }

            AppEvent::Quit => Ok(Transition::Quit),

            _ => Ok(Transition::Stay),
        }
    }
}

// ─── GameActionsScreen ────────────────────────────────────────────────────────

impl Screen for GameActionsScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        display.renderer.clear(DrawColor::White)?;

        let title = "Game Actions";
        let title_size = 56.0;
        let (tw, _) = display.renderer.measure_text(title, title_size);
        display.renderer.draw_text(
            (1072 - tw as i16) / 2,
            160,
            title,
            title_size,
            DrawColor::Black,
        )?;

        self.resign_button.draw(&mut display.renderer)?;
        self.abort_button.draw(&mut display.renderer)?;
        self.back_button.draw(&mut display.renderer)?;

        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        _display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            AppEvent::Touch(touch) => {
                if touch.kind != TouchKind::Up {
                    return Ok(Transition::Stay);
                }

                // Resign / abort fire-and-forget — the game-state stream still
                // running on ChessGameScreen picks up the terminal state and
                // flips the sidebar to "game over" once we pop back to it.
                if self.resign_button.rect.contains(touch.x, touch.y) {
                    if let Some(api) = self.app.online_in_game_api() {
                        info!("Resigning game");
                        tokio::spawn(async move {
                            if let Err(e) = api.resign_game().await {
                                warn!("resign_game failed: {}", e);
                            }
                        });
                    } else {
                        warn!("Resign with no in-game backend — ignored");
                    }
                    return Ok(Transition::Pop);
                }
                if self.abort_button.rect.contains(touch.x, touch.y) {
                    if let Some(api) = self.app.online_in_game_api() {
                        info!("Aborting game");
                        tokio::spawn(async move {
                            if let Err(e) = api.abort_game().await {
                                warn!("abort_game failed: {}", e);
                            }
                        });
                    } else {
                        warn!("Abort with no in-game backend — ignored");
                    }
                    return Ok(Transition::Pop);
                }
                if self.back_button.rect.contains(touch.x, touch.y) {
                    return Ok(Transition::Pop);
                }
                Ok(Transition::Stay)
            }

            AppEvent::Expose => Ok(Transition::Redraw),

            AppEvent::WindowUnmapped => {
                warn!("Window unmapped!");
                Ok(Transition::Stay)
            }

            AppEvent::Quit => Ok(Transition::Quit),

            _ => Ok(Transition::Stay),
        }
    }
}
