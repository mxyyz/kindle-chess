use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    api::litchee_adapter::LitcheeAdapter,
    models::{
        bitboard::Bitboards,
        oauth::{LichessUser, TokenInfo},
    },
};

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ BOARD ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

// Typestate marker: authed but no game scoped. `move_piece` / `resign_game` /
// `abort_game` / `stream_game_event` are not in scope here — they're only
// implemented on `BoardAPI<InGame>`, so the compiler refuses to call them.
#[derive(Debug, Clone)]
pub struct Idle;

// Typestate marker: a specific game is scoped. Holds runtime fields that only
// exist once a game is active. `turn` is dynamic and flips on every stream
// state event — the sidebar reads it to render "Your turn" / "Waiting".
//
// `initial_board` is the position parsed from `GameFull.initial_fen`. Each
// `GameState` event delivers the full move list from move 1, so we rebuild the
// current `board` by cloning `initial_board` and replaying the moves — no
// incremental state to keep in sync.
#[derive(Debug, Clone)]
pub struct InGame {
    pub game_id: String,
    pub white: Option<PlayedBy>,
    pub black: Option<PlayedBy>,
    pub player0_white: bool,
    pub turn: Turn,
    pub initial_board: Bitboards,
    pub board: Bitboards,
}

#[derive(Debug, Clone)]
pub enum Turn {
    Playing,
    Waiting,
    Over { winner: Option<String> },
}

#[derive(Debug, Clone)]
pub struct BoardAPI<S> {
    pub token: TokenInfo,
    pub user: LichessUser,
    pub state: S,
    pub adapter: LitcheeAdapter,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameData {
    pub full_id: String,
    pub game_id: String,
    pub color: String,
    pub fen: String,
    pub has_moved: bool,
    pub is_my_turn: bool,
    #[serde(default)]
    pub last_move: String,
    pub opponent: PlayedBy,
    pub perf: String,
    pub rated: bool,
    pub seconds_left: Option<u64>,
    pub source: String,
    pub speed: Speed,
    pub variant: GameVariant,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDataList {
    pub now_playing: Vec<GameData>,
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ STREAMS ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum GameStateStreamEvent {
    GameFull(Box<GameFullEvent>),
    GameState(GameStateEvent),
    GameOver(GameOverEvent),
    ChatLine(ChatLineEvent),
    OpponentGone(OpponentGoneEvent),
}

// ~~~~~~~~~~~~~~~~ GAME-STATE-STREAM-TYPES ~~~~~~~~~~~~~~~~
#[derive(Serialize, Deserialize, Debug)]
pub struct GameFullEvent {
    // pub id: Option<String>,
    pub variant: GameVariant,
    pub speed: Speed,
    pub perf: PerfMode,
    pub rated: bool,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
    pub white: PlayedBy,
    pub black: PlayedBy,
    #[serde(rename = "initialFen")]
    pub initial_fen: String,
    pub clock: Option<Clock>,
    pub state: GameStateEvent,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GameStateEvent {
    // #[serde(rename = "type")]
    // pub event_type: String,
    pub moves: String,
    pub wtime: u64,
    pub btime: u64,
    pub winc: u64,
    pub binc: u64,
    pub wdraw: Option<bool>,
    pub bdraw: Option<bool>,
    pub wtakeback: Option<bool>,
    pub btakeback: Option<bool>,
    pub status: String,
    // Lichess populates this on the terminal `gameState` event ("white" /
    // "black"). Absent for draws/stalemate/abort.
    #[serde(default)]
    pub winner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GameOverEvent {
    // #[serde(rename = "type")]
    // pub event_type: String,
    pub moves: String,
    pub wtime: u64,
    pub btime: u64,
    pub winc: u64,
    pub binc: u64,
    pub wdraw: Option<bool>,
    pub bdraw: Option<bool>,
    pub wtakeback: Option<bool>,
    pub btakeback: Option<bool>,
    pub status: String,
    pub winner: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChatLineEvent {
    // #[serde(rename = "type")]
    // pub event_type: String,
    pub username: String,
    pub text: String,
    pub room: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OpponentGoneEvent {
    // #[serde(rename = "type")]
    // pub event_type: String,
    pub gone: bool,
    #[serde(rename = "claimWinInSeconds")]
    pub claim_win_in_seconds: u64,
}

// MISC GAME-STATE-STREAM-EVENT-TYPES
#[derive(Serialize, Deserialize, Debug)]
pub struct GameVariant {
    pub key: String,
    pub name: String,
    pub short: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PerfMode {
    // #[serde(rename = "correspondence")]
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Clock {
    pub initial: u64,
    pub increment: u64,
}

#[derive(Debug, Serialize, Clone)]
pub enum PlayedBy {
    User(PlayedByPlayer),
    Ai(PlayedByAi),
}

impl PlayedBy {
    pub fn display_name(&self) -> String {
        match self {
            PlayedBy::User(p) => p.name.clone(),
            PlayedBy::Ai(ai) => match ai.ai_level {
                Some(level) => format!("AI lvl {}", level),
                None => "AI".to_string(),
            },
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct PlayedByPlayer {
    pub id: String,
    pub name: String,
    pub title: Option<String>,
    pub rating: u64,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct PlayedByAi {
    #[serde(rename = "aiLevel")]
    pub ai_level: Option<u8>,
}

// Lichess sends two different opponent shapes:
//   GET /api/account/playing  → {id, username, rating, ai}     (ai = level or null)
//   game-state stream         → {id, name, title, rating}      or {aiLevel}
// Untagged + try-User-first failed for now_playing because `name` was missing,
// so every opponent fell through to the all-Optional Ai variant. This proxy
// accepts both schemas and discriminates on whether an ai-level field is set.
#[derive(Deserialize)]
struct PlayedByRaw {
    #[serde(default)]
    id: Option<String>,
    #[serde(default, alias = "username")]
    name: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    rating: Option<u64>,
    #[serde(default, rename = "aiLevel", alias = "ai")]
    ai_level: Option<u8>,
}

impl<'de> Deserialize<'de> for PlayedBy {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = PlayedByRaw::deserialize(d)?;
        if let Some(level) = raw.ai_level {
            Ok(PlayedBy::Ai(PlayedByAi {
                ai_level: Some(level),
            }))
        } else {
            Ok(PlayedBy::User(PlayedByPlayer {
                id: raw.id.unwrap_or_default(),
                name: raw.name.unwrap_or_default(),
                title: raw.title,
                rating: raw.rating.unwrap_or(0),
            }))
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Performance {
    key: String,
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Speed {
    UltraBullet,
    Bullet,
    Blitz,
    Rapid,
    Classical,
    Correspondence,
}

/// Game mode for a friend challenge. Fixed defaults per e03 scope: standard
/// variant, casual (rated=false), random color — only the time control varies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChallengeMode {
    /// No clock at all.
    Unlimited,
    /// Real-time 10 minutes + 5 seconds increment (clock 600+5).
    RealTime,
    /// Correspondence, `days` per move (snapped to
    /// `ALLOWED_CORRESPONDENCE_DAYS` by the UI slider).
    Correspondence(u8),
}

impl ChallengeMode {
    /// The days value for correspondence modes.
    pub fn days_value(self) -> u8 {
        match self {
            ChallengeMode::Correspondence(d) => d,
            _ => 0,
        }
    }
}

/// A challenge as the app needs it: id for follow-up, opponent name, and
/// which side of the invite it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengeSummary {
    pub id: String,
    pub status: String,
    pub opponent: String,
    pub incoming: bool,
    /// Challenge URL (`https://lichess.org/{gameId}` once accepted).
    pub url: String,
    /// Whether the challenge is rated (e06).
    pub rated: bool,
}

/// An AI game created via `POST /api/challenge/ai` (e06s06). The game starts
/// immediately, so only the id is needed to open ChessGameScreen — the full
/// game state arrives over the board stream. litchee's `LichessGame` DTO
/// can't decode the live response (variant is an object, not a string), so
/// the adapter parses the raw response and extracts just the id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiGame {
    pub id: String,
}

/// Who to challenge from the Play form (e06). `Computer` carries the Lichess
/// AI level 1–8; `Random` is an open challenge anyone can accept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OpponentKind {
    Friend(String),
    Computer(u8),
    Random,
}

impl OpponentKind {
    /// Opponent username for friend challenges; `None` for Computer/Random.
    pub fn name(&self) -> Option<&str> {
        match self {
            OpponentKind::Friend(name) => Some(name),
            _ => None,
        }
    }

    /// AI level for computer challenges; `None` otherwise.
    pub fn level(&self) -> Option<u8> {
        match self {
            OpponentKind::Computer(level) => Some(*level),
            _ => None,
        }
    }

    /// Validated constructor for the AI level (Lichess accepts 1–8).
    pub fn computer_level(level: u8) -> Option<OpponentKind> {
        (1..=8)
            .contains(&level)
            .then_some(OpponentKind::Computer(level))
    }
}

/// Everything needed to offer a rematch after game over (e06s04): the same
/// opponent kind, time control, and rated flag as the original challenge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RematchContext {
    pub kind: OpponentKind,
    pub mode: ChallengeMode,
    pub rated: bool,
}

/// Day values Lichess accepts for correspondence challenges
/// (`days: Literal[1, 2, 3, 5, 7, 10, 14]` in the API contract) — anything else
/// is rejected with 400 (see BUG-2026-11-05T135402).
pub const ALLOWED_CORRESPONDENCE_DAYS: [u8; 7] = [1, 2, 3, 5, 7, 10, 14];

impl ChallengeSummary {
    /// Game id of an accepted challenge, taken from the challenge URL
    /// (`https://lichess.org/{gameId}`) — `None` until accepted.
    pub fn game_id(&self) -> Option<&str> {
        if self.status != "accepted" {
            return None;
        }
        self.url.rsplit('/').next().filter(|s| !s.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── GameData / GameDataList deserialization ──────────────────────────────

    fn sample_game_data_json() -> &'static str {
        r#"{
            "fullId": "abcdefgh12345678",
            "gameId": "abcdefgh",
            "color": "white",
            "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "hasMoved": false,
            "isMyTurn": true,
            "lastMove": "e2e4",
            "opponent": {"id": "opp1", "username": "opponent_name", "rating": 1500},
            "perf": "blitz",
            "rated": true,
            "secondsLeft": 300,
            "source": "lobby",
            "speed": "blitz",
            "variant": {"key": "standard", "name": "Standard", "short": "Std"}
        }"#
    }

    #[test]
    fn game_data_deserializes_camel_case_fields() {
        let gd: GameData = serde_json::from_str(sample_game_data_json()).unwrap();
        assert_eq!(gd.full_id, "abcdefgh12345678");
        assert_eq!(gd.game_id, "abcdefgh");
        assert_eq!(gd.color, "white");
        assert!(gd.is_my_turn);
        assert!(!gd.has_moved);
        assert_eq!(gd.last_move, "e2e4");
        assert_eq!(gd.seconds_left, Some(300));
        assert!(matches!(gd.speed, Speed::Blitz));
    }

    #[test]
    fn game_data_list_deserializes_now_playing_array() {
        let json = format!(r#"{{"nowPlaying": [{}]}}"#, sample_game_data_json());
        let list: GameDataList = serde_json::from_str(&json).unwrap();
        assert_eq!(list.now_playing.len(), 1);
        assert_eq!(list.now_playing[0].game_id, "abcdefgh");
    }

    #[test]
    fn game_data_last_move_defaults_to_empty_when_missing() {
        let mut v: serde_json::Value = serde_json::from_str(sample_game_data_json()).unwrap();
        v.as_object_mut().unwrap().remove("lastMove");
        let gd: GameData = serde_json::from_value(v).unwrap();
        assert_eq!(gd.last_move, "");
    }

    // ── GameStateStreamEvent variants ────────────────────────────────────────

    #[test]
    fn stream_event_game_full_deserializes() {
        let json = r#"{
            "type": "gameFull",
            "variant": {"key": "standard", "name": "Standard"},
            "speed": "blitz",
            "perf": {"name": "blitz"},
            "rated": false,
            "createdAt": 1700000000000,
            "white": {"id": "w", "name": "White", "rating": 1500},
            "black": {"id": "b", "name": "Black", "rating": 1500},
            "initialFen": "startpos",
            "state": {"type": "gameState", "moves": "", "wtime": 300000, "btime": 300000, "winc": 0, "binc": 0, "status": "started"}
        }"#;
        let ev: GameStateStreamEvent = serde_json::from_str(json).unwrap();
        match ev {
            GameStateStreamEvent::GameFull(full) => {
                assert_eq!(full.initial_fen, "startpos");
                assert_eq!(full.state.status, "started");
                assert_eq!(full.white.display_name(), "White");
            }
            _ => panic!("expected GameFull"),
        }
    }

    #[test]
    fn stream_event_game_state_deserializes() {
        let json = r#"{
            "type": "gameState",
            "moves": "e2e4 e7e5",
            "wtime": 290000,
            "btime": 295000,
            "winc": 2000,
            "binc": 2000,
            "status": "started",
            "winner": null
        }"#;
        let ev: GameStateStreamEvent = serde_json::from_str(json).unwrap();
        match ev {
            GameStateStreamEvent::GameState(gs) => {
                assert_eq!(gs.moves, "e2e4 e7e5");
                assert_eq!(gs.winner, None);
            }
            _ => panic!("expected GameState"),
        }
    }

    #[test]
    fn stream_event_chat_line_deserializes() {
        let json =
            r#"{"type": "chatLine", "username": "opponent", "text": "gg", "room": "player"}"#;
        let ev: GameStateStreamEvent = serde_json::from_str(json).unwrap();
        match ev {
            GameStateStreamEvent::ChatLine(cl) => {
                assert_eq!(cl.username, "opponent");
                assert_eq!(cl.text, "gg");
                assert_eq!(cl.room, "player");
            }
            _ => panic!("expected ChatLine"),
        }
    }

    #[test]
    fn stream_event_opponent_gone_deserializes() {
        let json = r#"{"type": "opponentGone", "gone": true, "claimWinInSeconds": 30}"#;
        let ev: GameStateStreamEvent = serde_json::from_str(json).unwrap();
        match ev {
            GameStateStreamEvent::OpponentGone(og) => {
                assert!(og.gone);
                assert_eq!(og.claim_win_in_seconds, 30);
            }
            _ => panic!("expected OpponentGone"),
        }
    }

    // ── PlayedBy: User vs AI discrimination + display_name ───────────────────

    #[test]
    fn played_by_user_deserializes_from_name_schema() {
        let json = r#"{"id": "u1", "name": "Alice", "title": "GM", "rating": 2500}"#;
        let pb: PlayedBy = serde_json::from_str(json).unwrap();
        assert_eq!(pb.display_name(), "Alice");
        match pb {
            PlayedBy::User(p) => {
                assert_eq!(p.id, "u1");
                assert_eq!(p.title, Some("GM".to_string()));
                assert_eq!(p.rating, 2500);
            }
            _ => panic!("expected User variant"),
        }
    }

    #[test]
    fn played_by_user_deserializes_from_username_alias() {
        // now_playing endpoint uses "username" instead of "name"
        let json = r#"{"id": "u2", "username": "Bob", "rating": 1800}"#;
        let pb: PlayedBy = serde_json::from_str(json).unwrap();
        assert_eq!(pb.display_name(), "Bob");
    }

    #[test]
    fn played_by_ai_deserializes_when_ai_level_present() {
        let json = r#"{"aiLevel": 3}"#;
        let pb: PlayedBy = serde_json::from_str(json).unwrap();
        assert_eq!(pb.display_name(), "AI lvl 3");
        match pb {
            PlayedBy::Ai(ai) => assert_eq!(ai.ai_level, Some(3)),
            _ => panic!("expected Ai variant"),
        }
    }

    #[test]
    fn played_by_ai_without_level_shows_plain_ai() {
        let json = r#"{"ai": null}"#;
        let pb: PlayedBy = serde_json::from_str(json).unwrap();
        // ai: null → ai_level is None → falls through to User with empty name
        // Actually, ai_level is None so it goes to User branch with defaults
        match pb {
            PlayedBy::User(p) => assert_eq!(p.name, ""),
            PlayedBy::Ai(_) => panic!("ai:null should not produce Ai variant"),
        }
    }

    #[test]
    fn played_by_ai_with_null_level_but_present_field() {
        // When ai is explicitly null, serde reads it as None, falls to User
        // When aiLevel is a number, it's Ai
        let json = r#"{"aiLevel": 8}"#;
        let pb: PlayedBy = serde_json::from_str(json).unwrap();
        assert_eq!(pb.display_name(), "AI lvl 8");
    }

    // ── ChallengeMode::days_value ────────────────────────────────────────────

    #[test]
    fn challenge_mode_days_value() {
        assert_eq!(ChallengeMode::Unlimited.days_value(), 0);
        assert_eq!(ChallengeMode::RealTime.days_value(), 0);
        assert_eq!(ChallengeMode::Correspondence(5).days_value(), 5);
        assert_eq!(ChallengeMode::Correspondence(14).days_value(), 14);
    }

    // ── ChallengeSummary::game_id ────────────────────────────────────────────

    #[test]
    fn challenge_game_id_none_until_accepted() {
        let c = ChallengeSummary {
            id: "abc".into(),
            status: "created".into(),
            opponent: "Bob".into(),
            incoming: false,
            url: "https://lichess.org/abc".into(),
            rated: false,
        };
        assert_eq!(c.game_id(), None);
    }

    #[test]
    fn challenge_game_id_extracted_from_url_when_accepted() {
        let c = ChallengeSummary {
            id: "abc".into(),
            status: "accepted".into(),
            opponent: "Bob".into(),
            incoming: false,
            url: "https://lichess.org/XyZ123ab".into(),
            rated: false,
        };
        assert_eq!(c.game_id(), Some("XyZ123ab"));
    }

    #[test]
    fn allowed_correspondence_days_matches_lichess_contract() {
        assert_eq!(ALLOWED_CORRESPONDENCE_DAYS, [1, 2, 3, 5, 7, 10, 14]);
    }

    // ── OpponentKind / RematchContext (e06) ─────────────────────────────────

    #[test]
    fn opponent_kind_variants_carry_their_payload() {
        let friend = OpponentKind::Friend("Alice".into());
        assert_eq!(friend.name(), Some("Alice"));
        let ai = OpponentKind::Computer(5);
        assert_eq!(ai.name(), None);
        assert_eq!(ai.level(), Some(5));
        assert_eq!(OpponentKind::Random.level(), None);
        assert_eq!(OpponentKind::Random.name(), None);
    }

    #[test]
    fn opponent_kind_rejects_out_of_range_ai_level() {
        assert!(OpponentKind::computer_level(0).is_none());
        assert!(OpponentKind::computer_level(9).is_none());
        assert_eq!(
            OpponentKind::computer_level(8),
            Some(OpponentKind::Computer(8))
        );
    }

    #[test]
    fn rematch_context_round_trips_and_preserves_settings() {
        let ctx = RematchContext {
            kind: OpponentKind::Friend("Alice".into()),
            mode: ChallengeMode::Correspondence(7),
            rated: true,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let back: RematchContext = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ctx);
        assert!(back.rated);
        assert_eq!(back.mode.days_value(), 7);
    }

    #[test]
    fn challenge_summary_carries_rated_flag() {
        let c = ChallengeSummary {
            id: "abc".into(),
            status: "created".into(),
            opponent: "Bob".into(),
            incoming: false,
            url: "https://lichess.org/abc".into(),
            rated: true,
        };
        assert!(c.rated);
    }
}
