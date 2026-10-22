use crate::api::oauth::authenticated_request;
use crate::app::game::player0_turn;
use crate::models::bitboard::Bitboards;
use crate::models::board_api::{
    BoardAPI, GameDataList, GameStateStreamEvent, Idle, InGame, PlayedBy, StreamEvent, Turn,
};
use crate::models::oauth::{HttpMethod, LichessUser, TokenInfo};
use crate::ui::events::AppEvent;
use futures::StreamExt;
use log::{info, warn};
use reqwest_streams::JsonStreamResponse;
use std::sync::mpsc::Sender;

// State-agnostic operations: ongoing-games list and the account event stream
// are valid in both Idle and InGame.
impl<S> BoardAPI<S> {
    pub async fn get_ongoing_games(
        &self,
        n: u8,
    ) -> Result<GameDataList, Box<dyn std::error::Error>> {
        let url = format!("{}/account/playing?nb={}", env!("LICHESS_API_BASE"), n);
        let response = authenticated_request(url, &self.token, HttpMethod::GET).await?;

        if !&response.status().is_success() {
            return Err(format!("Failed to retrieve ongoing games: {}", response.status()).into());
        }

        let bytes = response.bytes().await?;

        let data: GameDataList = serde_json::from_slice(&bytes).map_err(|e| {
            warn!(
                "Failed to parse JSON. Raw response: {}",
                String::from_utf8_lossy(&bytes)
            );
            warn!("Parse error: {}", e);
            e
        })?;
        info!("The received and parsed data {:?}", data);
        Ok(data)
    }

    pub async fn stream_event(&self) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/stream/event", env!("LICHESS_API_BASE"));

        info!("Getting event stream");
        let mut response = authenticated_request(url, &self.token, HttpMethod::GET)
            .await?
            .json_nl_stream::<StreamEvent>(1024);

        while let Some(result) = response.next().await {
            match result {
                Ok(event) => {
                    info!("Received event: {:?}", event);
                    self.handle_event(event).await?;
                }
                Err(e) => {
                    if e.to_string().contains("EOF while parsing") {
                        continue;
                    }
                    warn!("Error parsing event: {}", e);
                }
            }
        }

        Ok(())
    }

    pub async fn handle_event(&self, event: StreamEvent) -> Result<(), Box<dyn std::error::Error>> {
        match event {
            StreamEvent::GameStart(_) => info!("Issa GameStartEvent"),
            StreamEvent::GameFinish(_) => info!("Issa GameFinishEvent"),
            StreamEvent::Challenge(_) => info!("Issa ChallengeEvent"),
            StreamEvent::ChallengeDeclined(_) => info!("Issa ChallengeDeclinedEvent"),
        }
        Ok(())
    }
}

impl BoardAPI<Idle> {
    pub fn new(token: TokenInfo, user: LichessUser) -> Self {
        Self {
            token,
            user,
            state: Idle,
        }
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
        let url = format!(
            "{}/board/game/{}/move/{}",
            env!("LICHESS_API_BASE"),
            self.state.game_id,
            board_move
        );
        info!(
            "Sending move {} for game {}",
            board_move, self.state.game_id
        );

        let response = authenticated_request(url, &self.token, HttpMethod::POST).await?;

        if !response.status().is_success() {
            return Err(format!("Failed to move piece: {}", response.status()).into());
        }
        info!("Piece moved successfully");
        Ok(())
    }

    pub async fn resign_game(&self) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!(
            "{}/board/game/{}/resign",
            env!("LICHESS_API_BASE"),
            self.state.game_id
        );

        let response = authenticated_request(url, &self.token, HttpMethod::POST).await?;

        if !response.status().is_success() {
            return Err(format!("Failed to resign game: {}", response.status()).into());
        }
        info!("Game resigned");
        Ok(())
    }

    pub async fn abort_game(&self) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!(
            "{}/board/game/{}/abort",
            env!("LICHESS_API_BASE"),
            self.state.game_id
        );

        let response = authenticated_request(url, &self.token, HttpMethod::POST).await?;

        if !response.status().is_success() {
            return Err(format!("Failed to abort game: {}", response.status()).into());
        }
        info!("Game aborted");
        Ok(())
    }

    /// Open the game-state stream and drive it to completion.
    ///
    /// This runs inside a tokio task that owns its own clone of the API, so
    /// the local `&mut self` mutations to `self.state` are bookkeeping for
    /// computing whose turn it is on subsequent `GameState` events — they do
    /// **not** propagate back to the screen. Every state change the screen
    /// cares about is shipped as an `AppEvent` through `tx`. The screen
    /// applies those events to *its own* `ChessApp` copy.
    pub async fn stream_game_event(
        &mut self,
        tx: Sender<AppEvent>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!(
            "{}/board/game/stream/{}",
            env!("LICHESS_API_BASE"),
            self.state.game_id
        );
        info!("Game-state stream started for {}", self.state.game_id);

        let mut response = authenticated_request(url, &self.token, HttpMethod::STREAM)
            .await?
            .json_nl_stream::<GameStateStreamEvent>(1024);

        while let Some(result) = response.next().await {
            match result {
                Ok(event) => {
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
        Ok(())
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
                info!("Game is over. Winner is {}", over.winner);
                let turn = Turn::Over {
                    winner: Some(over.winner),
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
