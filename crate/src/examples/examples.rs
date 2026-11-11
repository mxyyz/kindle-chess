// DAILY-PUZZLE-TEST
// info!("Retreiving Daily Puzzle...");
// match get_daily_puzzle().await {
//     Ok(daily_puzzle) => {
//         info!("Retrieved Daily Puzzle!");
//         info!("Puzzle is {:?}", daily_puzzle.game);
//         info!("Some stats: {:?}", daily_puzzle.puzzle);
//     }
//     Err(e) => {
//         info!("Error retrieving puzzle: {}", e)
//     }
// }

// let mut auth_token: String = String::new();
// let game_id: String = String::from("LG4IZg4k");

// LOGOUT / TOKEN-DELETE-TEST
// match logout() {
//     Ok(()) => {
//         println!("Token deleted!")
//     }
//     Err(e) => {
//         println!("Token deletion error: {}", e)
//     }
// }

// BOARD-INTERACTION
// info!("Trying to abort game {}", game_id);
// match resign_game(&game_id, &auth_token).await {
//     Ok(()) => {
//         println!("Auth-request flow worked!");
//     }
//     Err(e) => {
//         eprintln!("Auth-request failed: {}", e);
//     }
// }

// move_piece(&game_id, &board_move)
//     .await
//     .unwrap();

// resign_game(&game_id, &auth_token).await.unwrap();

// RECENT GAMES
// let auth = get_authenticated().await.unwrap();

// // Get 5 most urgent games - assuming urgency = oldest / depending on gamemode
// let on_games = get_ongoing_games(&auth.0, 5).await.unwrap().now_playing;
// for game in &on_games {
//     info!("Retrieved game-id {}", &game.full_id);
// }
// let game_id = on_games[0].full_id.clone();
// info!("Streaming game id: {}", &game_id);
