//! One file per screen. Each module owns its struct, inherent impl,
//! `impl Screen`, helper functions, and tests.

pub mod chess_auth;
pub mod chess_game;
pub mod friendlist;
pub mod game_actions;
pub mod home;
pub mod invites;
pub mod ongoing_games;
pub mod play;
pub mod puzzle;
pub mod puzzle_settings;
pub mod settings;
pub mod update;

pub use chess_auth::ChessAuthScreen;
pub use chess_game::ChessGameScreen;
pub use friendlist::FriendlistScreen;
pub use game_actions::GameActionsScreen;
pub use home::HomeScreen;
pub use invites::InvitesScreen;
pub use ongoing_games::OngoingChessGamesScreen;
pub use play::PlayScreen;
pub use puzzle::PuzzleScreen;
pub use puzzle_settings::PuzzleSettingsScreen;
pub use settings::{SettingsContext, SettingsScreen};
pub use update::{UpdateScreen, UpdateState};

#[cfg(test)]
pub(crate) fn test_config() -> std::sync::Arc<std::sync::RwLock<crate::utils::config::AppConfig>> {
    std::sync::Arc::new(std::sync::RwLock::new(
        crate::utils::config::AppConfig::default(),
    ))
}
