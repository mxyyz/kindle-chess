use std::sync::{Arc, RwLock, mpsc::Sender};

use log::{info, warn};

use crate::i18n::{self, tr};

use super::*;

use crate::{
    api::client::{get_home_puzzle, get_next_puzzle, post_puzzle_solution},
    models::{
        oauth::TokenInfo,
        puzzle::{PuzzleParams, PuzzleSession, PuzzleStatus},
        ui::{Display, Screen, Transition},
    },
    ui::{
        events::{AppEvent, Rectangle, RectangleExt, Square},
        widgets::{BoardWidget, PuzzleSidebarWidget},
    },
    utils::config::AppConfig,
};

// ─── PuzzleScreen ─────────────────────────────────────────────────────────────
// Plays a Lichess puzzle. Unlike ChessGameScreen there is no stream: the puzzle
// is a self-contained position (`PuzzleSession`) and every move is validated
// locally against the scripted solution. Reuses the regular BoardWidget; the
// sidebar is the puzzle-specific one.

pub struct PuzzleScreen {
    pub board: BoardWidget,
    pub sidebar: PuzzleSidebarWidget,
    // None until the first puzzle has loaded (or while a reload is in flight).
    pub session: Option<PuzzleSession>,
    // OAuth token for authenticated `/api/puzzle/next` requests. None when the
    // user isn't signed in — daily/next still work anonymously.
    pub token: Option<TokenInfo>,
    // Last-used `/next` parameters; seeded into the settings screen and reused
    // by the sidebar's "Next puzzle" button.
    pub params: PuzzleParams,
    /// Shared app configuration. Seeds `params` at construction (so the
    /// user's last confirmed choices survive re-entering the puzzle flow) and
    /// is handed to the settings screen for persisting new choices.
    pub config: Arc<RwLock<AppConfig>>,
    // First-render guard: kick the daily-puzzle fetch exactly once.
    pub load_started: bool,
}

impl PuzzleScreen {
    pub fn new(token: Option<TokenInfo>, config: Arc<RwLock<AppConfig>>) -> Self {
        let params = config.read().map(|c| c.puzzle).unwrap_or_default();
        Self {
            board: BoardWidget::new(Rectangle::new(0, 0, 1072, 1072)),
            sidebar: PuzzleSidebarWidget::new(Rectangle::new(0, 1072, 1072, 376)),
            session: None,
            token,
            params,
            config,
            load_started: false,
        }
    }
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
                self.sidebar
                    .set_message(&tr(&i18n::locale_of(&self.config), "common-loading"));
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
                self.sidebar
                    .set_message(&tr(&i18n::locale_of(&self.config), "common-loading"));
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
