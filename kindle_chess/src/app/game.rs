use std::io;

pub fn player0_turn(moves: String, player0_white: bool) -> bool {
    // println!("Move-Count: {}", moves.split_whitespace().count());
    let white_turn = moves.split_whitespace().count().is_multiple_of(2);
    println!("White's turn? : {}", white_turn);
    // Return true if player is white and it's whites turn. Negate logic if player's black
    if player0_white {
        white_turn
    } else {
        !white_turn
    }
}

pub async fn get_turn_input() -> String {
    let mut buffer = String::new();
    println!("Enter move to play!");
    // TODO implement regex to sanitize
    let reader = io::stdin();
    match reader.read_line(&mut buffer) {
        Ok(_) => buffer,
        Err(e) => {
            println!("Failed to parse input: {}", e);
            String::new()
        }
    }
}
