//! Verify the app's GameDataList correctly parses a real /api/account/playing
//! response (litchee 0.2.1's LichessNowPlaying has schema bugs — variant is an
//! object not a string, nbMyTurn is required but often absent).
use kindli_chess::models::board_api::GameDataList;

#[test]
fn app_game_data_list_decodes_real_api_response() {
    // Per the Lichess API spec, `nbMyTurn` is a required field in the response.
    let real_response = r#"{
        "nbMyTurn": 1,
        "nowPlaying": [
            {
                "gameId": "hYOnBWerns",
                "fullId": "hYOnBWernsxYz",
                "color": "white",
                "fen": "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5Q2/PPPP1PPP/RNB1K1NR w KQkq - 4 4",
                "lastMove": "f1c4",
                "source": "lobby",
                "status": {"id": 20, "name": "started"},
                "variant": {"key": "standard", "name": "Standard", "short": "Std"},
                "speed": "blitz",
                "perf": "blitz",
                "rated": true,
                "hasMoved": false,
                "isMyTurn": true,
                "secondsLeft": 300,
                "opponent": {
                    "id": "someuser",
                    "username": "SomeUser",
                    "rating": 1500,
                    "ratingDiff": 5
                }
            }
        ]
    }"#;

    let result = serde_json::from_str::<GameDataList>(real_response);
    assert!(
        result.is_ok(),
        "GameDataList failed to decode: {:?}",
        result.err()
    );
    let list = result.unwrap();
    assert_eq!(list.now_playing.len(), 1);
    let game = &list.now_playing[0];
    assert_eq!(game.game_id, "hYOnBWerns");
    assert_eq!(game.full_id, "hYOnBWernsxYz");
    assert_eq!(game.color, "white");
    assert!(game.is_my_turn);
    assert_eq!(game.variant.key, "standard");
    assert_eq!(game.variant.name, "Standard");
}

#[test]
fn app_game_data_list_decodes_ai_opponent() {
    let response = r#"{
        "nbMyTurn": 1,
        "nowPlaying": [
            {
                "gameId": "abc12345",
                "fullId": "abc12345xyz",
                "color": "black",
                "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "source": "ai",
                "variant": {"key": "standard", "name": "Standard"},
                "speed": "rapid",
                "perf": "rapid",
                "rated": false,
                "hasMoved": false,
                "isMyTurn": true,
                "lastMove": "",
                "opponent": {
                    "id": null,
                    "username": "Stockfish level 4",
                    "ai": 4
                }
            }
        ]
    }"#;

    let result = serde_json::from_str::<GameDataList>(response);
    assert!(
        result.is_ok(),
        "AI opponent failed to decode: {:?}",
        result.err()
    );
    let list = result.unwrap();
    assert_eq!(list.now_playing.len(), 1);
}

#[test]
fn app_game_data_list_decodes_empty_response() {
    let response = r#"{ "nbMyTurn": 0, "nowPlaying": [] }"#;
    let result = serde_json::from_str::<GameDataList>(response);
    assert!(
        result.is_ok(),
        "Empty response failed to decode: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap().now_playing.len(), 0);
}
