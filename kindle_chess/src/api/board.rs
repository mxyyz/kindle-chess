use crate::api::litchee_adapter::LitcheeAdapter;
use crate::app::game::player0_turn;
use crate::models::bitboard::Bitboards;
use crate::models::board_api::{
    BoardAPI, ChallengeMode, ChallengeSummary, GameDataList, GameStateStreamEvent, Idle, InGame,
    PlayedBy, Turn,
};
use crate::models::oauth::{LichessUser, TokenInfo};
use crate::ui::events::AppEvent;
use futures_util::StreamExt;
use litchee::LichessError;
use log::{info, warn};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

/// How a `stream_game_event` run ended — the caller (reconnect loop) uses this
/// to decide whether/when to re-establish the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamEnd {
    /// The stream delivered at least one event. `terminal` is true when the
    /// last seen game state was a finished game — no point reconnecting.
    Events { terminal: bool },
    /// The stream closed without delivering any event.
    NoEvents,
}

// State-agnostic operations: ongoing-games list and the account event stream
// are valid in both Idle and InGame.
impl<S> BoardAPI<S> {
    pub async fn get_ongoing_games(
        &self,
        n: u8,
    ) -> Result<GameDataList, Box<dyn std::error::Error>> {
        Ok(self.adapter.get_ongoing_games(n).await?)
    }

    /// Usernames of followed players (Lichess friends) — the FriendlistScreen's
    /// data source for picking a challenge opponent.
    pub async fn get_following(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        Ok(self.adapter.get_following().await?)
    }

    /// Send a friend challenge with the given game mode.
    pub async fn send_challenge(
        &self,
        username: &str,
        mode: &ChallengeMode,
    ) -> Result<ChallengeSummary, Box<dyn std::error::Error>> {
        Ok(self.adapter.send_challenge(username, mode).await?)
    }

    /// Incoming + outgoing challenges (InvitesScreen data source).
    pub async fn list_challenges(
        &self,
    ) -> Result<Vec<ChallengeSummary>, Box<dyn std::error::Error>> {
        Ok(self.adapter.list_challenges().await?)
    }

    pub async fn accept_challenge(
        &self,
        challenge_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(self.adapter.accept_challenge(challenge_id).await?)
    }

    pub async fn decline_challenge(
        &self,
        challenge_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(self.adapter.decline_challenge(challenge_id).await?)
    }

    pub async fn cancel_challenge(
        &self,
        challenge_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(self.adapter.cancel_challenge(challenge_id).await?)
    }
}

impl BoardAPI<Idle> {
    pub fn new(token: TokenInfo, user: LichessUser) -> Result<Self, LichessError> {
        let adapter = LitcheeAdapter::new(&token.access_token)?;
        Ok(Self {
            token,
            user,
            state: Idle,
            adapter,
        })
    }

    /// Consume the idle API and produce an in-game one scoped to `game_id`.
    /// `my_turn` is the snapshot from `GameData.is_my_turn` at attach time so
    /// the sidebar can render something before the game-state stream catches up.
    pub fn attach_game(self, game_id: String, my_turn: bool) -> BoardAPI<InGame> {
        let starting = Bitboards::starting_position();
        BoardAPI {
            token: self.token,
            user: self.user,
            state: InGame {
                game_id,
                white: None,
                black: None,
                player0_white: false,
                turn: if my_turn {
                    Turn::Playing
                } else {
                    Turn::Waiting
                },
                initial_board: starting.clone(),
                board: starting,
            },
            adapter: self.adapter,
        }
    }
}

impl BoardAPI<InGame> {
    pub fn game_id(&self) -> &str {
        &self.state.game_id
    }

    pub fn turn(&self) -> &Turn {
        &self.state.turn
    }

    pub async fn move_piece(&self, board_move: &str) -> Result<(), Box<dyn std::error::Error>> {
        Ok(self.adapter.move_piece(self.game_id(), board_move).await?)
    }

    pub async fn resign_game(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(self.adapter.resign_game(self.game_id()).await?)
    }

    pub async fn abort_game(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(self.adapter.abort_game(self.game_id()).await?)
    }

    /// Open the game-state stream and drive it to completion.
    ///
    /// This runs inside a tokio task that owns its own clone of the API, so
    /// the local `&mut self` mutations to `self.state` are bookkeeping for
    /// computing whose turn it is on subsequent `GameState` events — they do
    /// **not** propagate back to the screen. Every state change the screen
    /// cares about is shipped as an `AppEvent` through `tx`. The screen
    /// applies those events to *its own* `ChessApp` copy.
    ///
    /// `cancel`: when set, the task stops forwarding events and exits. Used to
    /// supersede a stalled stream (e.g. after device wake) with a fresh one —
    /// Lichess sends the full game state on every new stream connection, so
    /// re-kicking is also the refetch path.
    pub async fn stream_game_event(
        &mut self,
        tx: Sender<AppEvent>,
        cancel: Arc<AtomicBool>,
    ) -> Result<StreamEnd, Box<dyn std::error::Error>> {
        let mut stream = self.adapter.stream_game(self.game_id()).await?;

        let mut got_any = false;
        let mut terminal = false;
        while let Some(result) = stream.next().await {
            if cancel.load(Ordering::SeqCst) {
                info!("game stream cancelled (superseded by a newer stream)");
                break;
            }
            match result {
                Ok(event) => {
                    got_any = true;
                    terminal = match &event {
                        GameStateStreamEvent::GameFull(f) => is_terminal_status(&f.state.status),
                        GameStateStreamEvent::GameState(s) => is_terminal_status(&s.status),
                        GameStateStreamEvent::GameOver(_) => true,
                        _ => terminal,
                    };
                    info!("Received event: {:?}", event);
                    if let Err(e) = self.handle_game_event(event, &tx).await {
                        warn!("Error while handling game event: {e}");
                    }
                }
                Err(e) => {
                    // Ignore the stream ping (= empty line)
                    if e.to_string().contains("EOF while parsing") {
                        continue;
                    }
                    warn!("Error parsing event: {}", e);
                }
            }
        }
        Ok(if got_any {
            StreamEnd::Events { terminal }
        } else {
            StreamEnd::NoEvents
        })
    }

    async fn handle_game_event(
        &mut self,
        event: GameStateStreamEvent,
        tx: &Sender<AppEvent>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match event {
            GameStateStreamEvent::GameFull(full) => {
                let player0_white =
                    matches!(&full.white, PlayedBy::User(p) if p.id == self.user.id);
                let my_turn = player0_turn(full.state.moves.clone(), player0_white);
                let turn = if is_terminal_status(&full.state.status) {
                    Turn::Over {
                        winner: resolve_winner(
                            full.state.winner.as_deref(),
                            Some(&full.white),
                            Some(&full.black),
                        ),
                    }
                } else if my_turn {
                    Turn::Playing
                } else {
                    Turn::Waiting
                };

                // Initial position from `initial_fen` (usually the standard
                // start, but Lichess hands us the FEN explicitly so chess960
                // and odds games work too). Replay the move list to catch up
                // if we joined mid-game.
                let initial_board = match Bitboards::from_fen(&full.initial_fen) {
                    Ok(bb) => bb,
                    Err(e) => {
                        warn!(
                            "Bad initial FEN '{}': {} — falling back to start",
                            full.initial_fen, e
                        );
                        Bitboards::starting_position()
                    }
                };
                let mut board = initial_board.clone();
                board.apply_uci_moves(&full.state.moves);
                let last_move = last_move_mask(&initial_board, &full.state.moves);

                // Update local bookkeeping so subsequent GameState events can
                // resolve whose-turn-it-is from `player0_white` and rebuild
                // the position from `initial_board`.
                self.state.white = Some(full.white.clone());
                self.state.black = Some(full.black.clone());
                self.state.player0_white = player0_white;
                self.state.turn = turn.clone();
                self.state.initial_board = initial_board;
                self.state.board = board.clone();

                let _ = tx.send(AppEvent::GameFullReceived {
                    white: full.white,
                    black: full.black,
                    player0_white,
                    turn,
                    board,
                    last_move,
                });
            }
            GameStateStreamEvent::GameState(state) => {
                let my_turn = player0_turn(state.moves.clone(), self.state.player0_white);
                let turn = if is_terminal_status(&state.status) {
                    Turn::Over {
                        winner: resolve_winner(
                            state.winner.as_deref(),
                            self.state.white.as_ref(),
                            self.state.black.as_ref(),
                        ),
                    }
                } else if my_turn {
                    Turn::Playing
                } else {
                    Turn::Waiting
                };

                // Each GameState carries the full move list from move 1, so
                // rebuild from `initial_board` rather than tracking deltas.
                let mut board = self.state.initial_board.clone();
                board.apply_uci_moves(&state.moves);
                let last_move = last_move_mask(&self.state.initial_board, &state.moves);

                self.state.turn = turn.clone();
                self.state.board = board.clone();
                let _ = tx.send(AppEvent::TurnChanged {
                    turn,
                    board,
                    last_move,
                });
            }
            GameStateStreamEvent::GameOver(over) => {
                // Lichess omits `winner` for draws / stalemate / abort; the
                // adapter maps a missing winner to an empty string. Treat ""
                // as "no winner" so the sidebar shows "Game over", not " won!".
                let turn = Turn::Over {
                    winner: if over.winner.is_empty() {
                        None
                    } else {
                        Some(over.winner)
                    },
                };

                // Final position from the over event's own move list — the
                // mating move can land in either GameState or GameOver, so we
                // rebuild rather than trusting the last GameState we saw.
                let mut board = self.state.initial_board.clone();
                board.apply_uci_moves(&over.moves);

                let last_move = last_move_mask(&self.state.initial_board, &over.moves);
                self.state.turn = turn.clone();
                self.state.board = board.clone();
                let _ = tx.send(AppEvent::TurnChanged {
                    turn,
                    board,
                    last_move,
                });
            }
            GameStateStreamEvent::ChatLine(_) => info!("Issa ChatlineEvent"),
            GameStateStreamEvent::OpponentGone(_) => info!("Issa OpponentGoneEvent"),
        }
        Ok(())
    }
}

// Lichess marks an in-progress game as `created` (no moves yet) or `started`.
// Anything else (mate, resign, stalemate, draw, outoftime, aborted, ...) is
// terminal — the screen should flip to a "Game over" state.
fn is_terminal_status(status: &str) -> bool {
    !matches!(status, "started" | "created")
}

// Bitmask of squares affected by the last UCI move. Computed by replaying
// every move except the last, then diffing against the post-last-move
// position — that catches castling rook squares and en-passant captured
// pawns without special-casing UCI string shapes.
fn last_move_mask(initial: &Bitboards, moves_str: &str) -> u64 {
    let moves: Vec<&str> = moves_str.split_whitespace().collect();
    if moves.is_empty() {
        return 0;
    }
    let split = moves.len() - 1;

    let mut prev = initial.clone();
    for mv in &moves[..split] {
        let _ = prev.apply_uci_move(mv);
    }
    let mut curr = prev.clone();
    if curr.apply_uci_move(moves[split]).is_err() {
        return 0;
    }

    let mut mask = 0u64;
    for sq in 0..64u8 {
        if prev.piece_at(sq) != curr.piece_at(sq) {
            mask |= 1u64 << sq;
        }
    }
    mask
}

// Lichess sends the winning *side* ("white" / "black") on a terminal
// `gameState`; map that back to the corresponding player's display name.
// Returns None for draws/stalemate/abort (no winner field on the wire) or if
// the player slot isn't populated yet.
fn resolve_winner(
    winner_color: Option<&str>,
    white: Option<&PlayedBy>,
    black: Option<&PlayedBy>,
) -> Option<String> {
    match winner_color? {
        "white" => white.map(PlayedBy::display_name),
        "black" => black.map(PlayedBy::display_name),
        _ => None,
    }
}

#[cfg(test)]
mod typestate_tests {
    use super::*;
    use crate::models::oauth::TokenInfo;

    fn test_token() -> TokenInfo {
        TokenInfo {
            access_token: "test_token_12345".into(),
            token_type: "Bearer".into(),
            expires_in: Some(86400),
            scope: None,
        }
    }

    fn test_user() -> LichessUser {
        LichessUser {
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
        }
    }

    #[test]
    fn idle_new_creates_board_api() {
        let api = BoardAPI::<Idle>::new(test_token(), test_user()).unwrap();
        // Idle state has no game_id — that's the whole point of the typestate.
        let _: &Idle = &api.state;
    }

    #[test]
    fn attach_game_transitions_to_in_game() {
        let idle = BoardAPI::<Idle>::new(test_token(), test_user()).unwrap();
        let in_game = idle.attach_game("game123abc".into(), true);
        assert_eq!(in_game.game_id(), "game123abc");
    }

    #[test]
    fn attach_game_sets_turn_playing_when_my_turn() {
        let idle = BoardAPI::<Idle>::new(test_token(), test_user()).unwrap();
        let in_game = idle.attach_game("g1".into(), true);
        assert!(matches!(in_game.turn(), Turn::Playing));
    }

    #[test]
    fn attach_game_sets_turn_waiting_when_not_my_turn() {
        let idle = BoardAPI::<Idle>::new(test_token(), test_user()).unwrap();
        let in_game = idle.attach_game("g2".into(), false);
        assert!(matches!(in_game.turn(), Turn::Waiting));
    }

    #[test]
    fn in_game_state_has_starting_board() {
        let idle = BoardAPI::<Idle>::new(test_token(), test_user()).unwrap();
        let in_game = idle.attach_game("g3".into(), true);
        // initial_board should be the standard starting position
        let starting = Bitboards::starting_position();
        assert_eq!(in_game.state.initial_board.boards, starting.boards);
        assert_eq!(in_game.state.board.boards, starting.boards);
    }

    // Compile-fail cases (documented, not tested with trybuild):
    // - BoardAPI::<Idle> has no move_piece/resign_game/abort_game/stream_game_event
    //   → calling api.move_piece("e2e4") on Idle is a compile error
    // - BoardAPI::<InGame> has no attach_game → calling in_game.attach_game(...) is
    //   a compile error (attach_game consumes self and is only on Idle)
    // - Idle has no game_id/turn accessors → api.game_id() on Idle is a compile error
}
