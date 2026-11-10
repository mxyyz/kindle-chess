use serde::{Deserialize, Serialize};

use crate::models::board_api::Performance;

// ~~~~~~~~~~~~~~~~ GAME ~~~~~~~~~~~~~~~~
#[derive(Serialize, Deserialize, Debug)]
pub struct Game {
    id: String,
    perf: Performance,
    rated: bool,
    players: Vec<Player>,
    pgn: String,
    clock: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Player {
    name: String,
    id: String,
    color: String,
    rating: i32,
    //patron: Option<bool>,
    flare: Option<String>,
    online: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_deserializes_from_json() {
        let json = r#"{
            "id": "game1",
            "perf": {"key": "blitz", "name": "Blitz"},
            "rated": true,
            "players": [
                {"name": "Alice", "id": "a1", "color": "white", "rating": 1500, "flare": "fl", "online": true},
                {"name": "Bob", "id": "b1", "color": "black", "rating": 1600}
            ],
            "pgn": "1. e4 e5",
            "clock": "300+2"
        }"#;
        let g: Game = serde_json::from_str(json).unwrap();
        // Fields are private — verify deserialization succeeds by re-serializing
        let out = serde_json::to_string(&g).unwrap();
        assert!(out.contains("game1"));
        assert!(out.contains("Alice"));
        assert!(out.contains("Bob"));
    }

    #[test]
    fn player_deserializes_minimal_fields() {
        let json = r#"{"name": "Solo", "id": "s1", "color": "white", "rating": 1200}"#;
        let p: Player = serde_json::from_str(json).unwrap();
        let out = serde_json::to_string(&p).unwrap();
        assert!(out.contains("Solo"));
        assert!(out.contains("1200"));
    }

    #[test]
    fn player_optional_fields_default_to_none() {
        let json = r#"{"name": "X", "id": "x1", "color": "black", "rating": 800}"#;
        let p: Player = serde_json::from_str(json).unwrap();
        let out = serde_json::to_string(&p).unwrap();
        // flare and online are Option — should serialize as null or be absent
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.get("flare").is_none() || v["flare"].is_null());
        assert!(v.get("online").is_none() || v["online"].is_null());
    }
}
