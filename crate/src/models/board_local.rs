#[derive(Debug, Clone)]
pub struct BoardLocal {
    // pub game_id: String,
    pub player0_white: bool, // if true then player0 had first turn
    pub player0_turn: bool,  // if true then it's currently player0's turn
}
