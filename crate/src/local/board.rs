use log::info;

use crate::models::board_local::BoardLocal;

impl BoardLocal {
    pub fn new(game_id: String) -> BoardLocal {
        println!(
            "Would start local game {}... If it were implemented",
            game_id,
        );

        let turn = true; // TODO: get turn from local savegame
        let white = true; // TODO: get player0 color from local savegame

        Self {
            // game_id: game_id,
            player0_turn: turn,
            player0_white: white,
        }
    }
    pub async fn get_ongoing_games() {
        info!("Not implemented")
    }
}
