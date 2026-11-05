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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeMode {
    /// No clock at all.
    Unlimited,
    /// Real-time 10 minutes + 5 seconds increment (clock 600+5).
    RealTime,
    /// Correspondence, `days` per move (clamped 1..=14 by the UI spinner).
    Correspondence(u8),
}

/// A challenge as the app needs it: id for follow-up, opponent name, and
/// which side of the invite it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengeSummary {
    pub id: String,
    pub status: String,
    pub opponent: String,
    pub incoming: bool,
}
