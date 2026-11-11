use litchee::LichessError;

use crate::models::{
    board_api::{BoardAPI, Idle, InGame, PlayedBy, Turn},
    board_local::BoardLocal,
    chess::{ChessApp, ChessBackend},
    oauth::{LichessUser, TokenInfo},
};
use log::warn;

impl ChessApp {
    /// Constructs an online backend in the `Idle` state. No game is scoped
    /// yet — call `attach_game` after picking one from the ongoing-games
    /// list. Fails if the Lichess client can't be built.
    pub fn new_online(token: TokenInfo, user: LichessUser) -> Result<ChessApp, LichessError> {
        Ok(Self {
            backend: ChessBackend::OnlineIdle(Box::new(BoardAPI::<Idle>::new(token, user)?)),
        })
    }

    pub fn new_offline() -> ChessApp {
        let board_local = BoardLocal::new(String::new());
        Self {
            backend: ChessBackend::Offline(board_local),
        }
    }

    /// Scope the underlying API to `game_id`, producing an `InGame` backend.
    /// `my_turn` is the snapshot from `GameData.is_my_turn` (or the AI
    /// rematch's `true`) so the sidebar can render an initial turn status
    /// before the game-state stream takes over.
    ///
    /// Works from `Idle` (picking a game) AND from an already in-game backend
    /// (rematch — BUG-2026-11-11T145208): the in-game scope is detached and
    /// re-attached to the new game id instead of being ignored.
    pub fn attach_game(mut self, game_id: String, my_turn: bool) -> Self {
        self.backend = match self.backend {
            ChessBackend::OnlineIdle(api) => {
                ChessBackend::OnlineInGame(Box::new(api.attach_game(game_id, my_turn)))
            }
            ChessBackend::OnlineInGame(api) => {
                ChessBackend::OnlineInGame(Box::new(api.detach().attach_game(game_id, my_turn)))
            }
            other => {
                warn!("attach_game called on offline backend; ignored");
                other
            }
        };
        self
    }

    /// Cloned `BoardAPI<Idle>` for use in tasks that only need read access to
    /// the auth scope (e.g. ongoing-games fetch).
    pub fn online_idle_api(&self) -> Option<BoardAPI<Idle>> {
        match &self.backend {
            ChessBackend::OnlineIdle(api) => Some((**api).clone()),
            _ => None,
        }
    }

    /// Cloned `BoardAPI<InGame>` for use in tasks scoped to a specific game
    /// (game-state stream, move submission). The clone's runtime mutations
    /// stay inside the task — propagate state changes back via `AppEvent`s.
    pub fn online_in_game_api(&self) -> Option<BoardAPI<InGame>> {
        match &self.backend {
            ChessBackend::OnlineInGame(api) => Some((**api).clone()),
            _ => None,
        }
    }

    pub fn turn(&self) -> Option<&Turn> {
        match &self.backend {
            ChessBackend::OnlineInGame(api) => Some(&api.state.turn),
            _ => None,
        }
    }

    /// The OAuth token backing this app, if any. Used to make authenticated
    /// `/api/puzzle/next` requests; an offline backend has no token and the
    /// puzzle screen then falls back to anonymous puzzle fetches.
    pub fn token(&self) -> Option<TokenInfo> {
        match &self.backend {
            ChessBackend::OnlineIdle(api) => Some(api.token.clone()),
            ChessBackend::OnlineInGame(api) => Some(api.token.clone()),
            ChessBackend::Offline(_) => None,
        }
    }

    /// Apply the initial `GameFull` snapshot to the in-game state. No-op for
    /// non-in-game backends.
    pub fn apply_game_full(
        &mut self,
        white: PlayedBy,
        black: PlayedBy,
        player0_white: bool,
        turn: Turn,
    ) {
        if let ChessBackend::OnlineInGame(api) = &mut self.backend {
            api.state.white = Some(white);
            api.state.black = Some(black);
            api.state.player0_white = player0_white;
            api.state.turn = turn;
        }
    }

    pub fn apply_turn(&mut self, turn: Turn) {
        if let ChessBackend::OnlineInGame(api) = &mut self.backend {
            api.state.turn = turn;
        }
    }
}

#[cfg(test)]
mod attach_game_tests {
    use super::*;
    use crate::models::oauth::{LichessUser, TokenInfo};

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
    fn attach_game_from_idle_scopes_game() {
        let app = ChessApp::new_online(test_token(), test_user()).unwrap();
        let app = app.attach_game("gameA".into(), true);
        let api = app.online_in_game_api().unwrap();
        assert_eq!(api.game_id(), "gameA");
        assert!(matches!(api.turn(), Turn::Playing));
    }

    // BUG-2026-11-11T145208: rematch attaches a NEW game while the app is
    // already in-game (old game). The transition must re-scope to the new
    // game id instead of ignoring the call.
    #[test]
    fn attach_game_from_in_game_rescopes_to_new_game() {
        let app = ChessApp::new_online(test_token(), test_user()).unwrap();
        let app = app.attach_game("oldGame".into(), true);
        assert_eq!(app.online_in_game_api().unwrap().game_id(), "oldGame");

        let app = app.attach_game("newGame".into(), false);
        let api = app.online_in_game_api().unwrap();
        assert_eq!(
            api.game_id(),
            "newGame",
            "rematch must re-scope the in-game backend to the new game id"
        );
        assert!(matches!(api.turn(), Turn::Waiting));
    }

    #[test]
    fn attach_game_on_offline_backend_stays_offline() {
        let app = ChessApp::new_offline();
        let app = app.attach_game("gameX".into(), true);
        assert!(app.online_in_game_api().is_none());
        assert!(app.online_idle_api().is_none());
    }
}
