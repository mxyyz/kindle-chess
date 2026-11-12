use std::pin::Pin;

use futures_util::{Stream, StreamExt};
use litchee::{
    LichessError,
    api::gameplay::{
        board::{LichessBoardEvent, LichessGameEventPlayer, LichessGameState, LichessVariantInfo},
        challenges::{LichessChallenge, LichessChallengeColor},
        games::LichessGameStatusName,
    },
    model::{LichessColor, LichessTitle, LichessVariantKey},
};

use log::warn;

use crate::models::{
    board_api::{
        AiGame, ChallengeMode, ChallengeSummary, ChatLineEvent, Clock, GameDataList, GameFullEvent,
        GameOverEvent, GameStateEvent, GameStateStreamEvent, GameVariant, OpponentGoneEvent,
        PerfMode, PlayedBy, PlayedByAi, PlayedByPlayer, Speed,
    },
    oauth::{LichessUser, UserProfile},
    puzzle::{Puzzle, PuzzleColor, PuzzleParams, PuzzleSolveRound},
};
#[derive(Clone, Debug)]
pub struct LitcheeAdapter {
    client: litchee::LichessClient,
    /// Raw access token, kept for endpoints where litchee's DTOs don't match
    /// the live API (e.g. `GET /api/account/playing` — litchee 0.2.1 expects
    /// `variant` as a string but the API returns an object, and requires
    /// `nbMyTurn` which the API omits).
    token: Option<String>,
    /// Base URL for the raw-reqwest endpoints (`{base}/account/playing`,
    /// `{base}/challenge/ai`). Stored so tests can point the adapter at a
    /// mock server via `with_base_url`.
    base_url: String,
}

impl LitcheeAdapter {
    pub fn new(token: &str) -> Result<Self, LichessError> {
        let client = litchee::LichessClient::builder().token(token).build()?;
        Ok(Self {
            client,
            token: Some(token.to_string()),
            base_url: env!("LICHESS_API_BASE").to_string(),
        })
    }

    pub fn anonymous() -> Self {
        Self {
            client: litchee::LichessClient::new(),
            token: None,
            base_url: env!("LICHESS_API_BASE").to_string(),
        }
    }

    /// Builds an adapter against a custom base URL, optionally authenticated.
    ///
    /// Meant for integration tests: `LichessClientBuilder::base_url()` lets a
    /// mock server stand in for `lichess.org`, so the adapter's full
    /// request → decode → DTO-mapping path can be exercised offline.
    pub fn with_base_url(token: Option<&str>, base_url: &url::Url) -> Result<Self, LichessError> {
        let mut builder = litchee::LichessClient::builder().base_url(base_url);
        let token = match token {
            Some(t) => {
                builder = builder.token(t);
                Some(t.to_string())
            }
            None => None,
        };
        // litchee treats `base_url` as the host root and prefixes `/api` on
        // every request path itself. Normalize the stored base the same way
        // so the raw-reqwest endpoints (`{base}/challenge/ai`, …) resolve to
        // the same URLs litchee would hit.
        let mut base = base_url.as_str().trim_end_matches('/').to_string();
        if base_url.path().is_empty() || base_url.path() == "/" {
            base.push_str("/api");
        }
        Ok(Self {
            client: builder.build()?,
            token,
            base_url: base,
        })
    }

    // Puzzle methods
    pub async fn get_daily_puzzle(&self) -> Result<Puzzle, LichessError> {
        let daily = self.client.puzzles().daily().await?;
        let daily_puzzle = Puzzle {
            id: daily.puzzle.id,
            rating: daily.puzzle.rating.unwrap_or(0) as i32,
            plays: daily.puzzle.plays.unwrap_or(0) as i32,
            solution: daily.puzzle.solution,
            themes: daily.puzzle.themes,
            initial_ply: daily.puzzle.initial_ply.unwrap_or(0) as i32,
            fen: daily.puzzle.fen,
            last_move: daily.puzzle.last_move,
        };
        Ok(daily_puzzle)
    }

    pub async fn get_puzzle_by_id(&self, id: &str) -> Result<Puzzle, LichessError> {
        let response = self.client.puzzles().get(id).await?;
        let puzzle = Puzzle {
            id: response.puzzle.id,
            rating: response.puzzle.rating.unwrap_or(0) as i32,
            plays: response.puzzle.plays.unwrap_or(0) as i32,
            solution: response.puzzle.solution,
            themes: response.puzzle.themes,
            initial_ply: response.puzzle.initial_ply.unwrap_or(0) as i32,
            fen: response.puzzle.fen,
            last_move: response.puzzle.last_move,
        };
        Ok(puzzle)
    }

    pub async fn get_next_puzzle(&self, params: &PuzzleParams) -> Result<Puzzle, LichessError> {
        let color = match params.color {
            PuzzleColor::Random => None,
            PuzzleColor::White => Some(litchee::model::LichessColor::White),
            PuzzleColor::Black => Some(litchee::model::LichessColor::Black),
        };
        let response = self
            .client
            .puzzles()
            .next(
                params.phase.as_angle(),
                Some(params.difficulty.as_param()),
                color,
            )
            .await?;
        // the next-endpoint of the api returns puzzles without fen. Refech the puzzle for full data
        let puzzle = self.get_puzzle_by_id(&response.puzzle.id).await?;
        Ok(puzzle)
    }

    /// POST /api/puzzle/batch/{angle} — submit one solved puzzle to the
    /// account. `nb > 0` additionally asks Lichess for a fresh batch in the
    /// response (not used here; "Next puzzle" goes through /puzzle/next).
    /// Requires a token with the `puzzle:write` scope — older tokens get a
    /// 403, which surfaces as `Err` so the caller can degrade quietly.
    pub async fn post_puzzle_solution(
        &self,
        angle: &str,
        id: &str,
        win: bool,
        nb: u32,
    ) -> Result<Vec<PuzzleSolveRound>, LichessError> {
        use litchee::api::training::puzzles::LichessPuzzleSolution;
        let mut solution = LichessPuzzleSolution::new(id, win);
        solution.rated = Some(true);
        let response = self
            .client
            .puzzles()
            .solve_batch(angle, &[solution], nb)
            .await?;
        Ok(response
            .rounds
            .iter()
            .map(|r| PuzzleSolveRound {
                id: r.id.clone(),
                win: r.win,
                rating_diff: r.rating_diff,
            })
            .collect())
    }

    pub async fn get_home_puzzle(&self, params: &PuzzleParams) -> Result<Puzzle, LichessError> {
        // Gets activity of the last 24h and checks for the daily puzzle's ID in there
        // Retuns the daily puzzle if not found; the next puzzle if found
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| LichessError::InvalidRequest(format!("clock error: {e}")))?
            .as_millis() as i64;
        let since_24h_ago = now_ms - 86_400_000;
        let daily = self.get_daily_puzzle().await?;
        let mut activity = self
            .client
            .puzzles()
            .activity(None, None, Some(since_24h_ago))
            .await?;
        while let Some(entry) = activity.next().await {
            let entry = entry?;
            if entry.puzzle.id == daily.id {
                // get_next_puzzle with some params
                return self.get_next_puzzle(params).await;
            }
        }
        Ok(daily)
    }

    // Account
    pub async fn get_user_info(&self) -> Result<LichessUser, LichessError> {
        let litchee_user = self.client.account().profile().await?.user;

        // profile: Option<LichessProfile> → Option<UserProfile>
        let profile = litchee_user.profile.map(|p| UserProfile {
            country: p.flag,
            location: p.location,
            bio: p.bio,
            first_name: p.real_name.clone(),
            last_name: p.real_name,
            links: p.links,
        });

        // title: Option<LichessTitle> → Option<String>
        let title = title_to_string(litchee_user.title);

        // perfs: Option<LichessPerfs> → Option<serde_json::Value>
        let perfs = litchee_user
            .perfs
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| LichessError::InvalidRequest(format!("perfs serialization: {e}")))?;

        // play_time: Option<LichessPlayTime> → Option<serde_json::Value>
        let play_time = litchee_user
            .play_time
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| LichessError::InvalidRequest(format!("play_time serialization: {e}")))?;

        let user = LichessUser {
            id: litchee_user.id,
            username: litchee_user.username,
            perfs,
            created_at: litchee_user.created_at,
            disabled: litchee_user.disabled,
            tos_violation: litchee_user.tos_violation,
            profile,
            seen_at: litchee_user.seen_at,
            patron: litchee_user.patron,
            verified: litchee_user.verified,
            play_time,
            title,
        };
        Ok(user)
    }

    // Games
    //
    // NOTE: This bypasses litchee and uses reqwest directly. litchee 0.2.1's
    // `LichessNowPlaying` type has two schema bugs vs. the live API:
    //   1. `variant` is typed as `LichessVariantKey` (a string enum), but the
    //      API returns an object `{"key": "standard", "name": "Standard", ...}`.
    //   2. `nb_my_turn` is required (no `#[serde(default)]`), but the API omits it.
    // The app's own `GameDataList` / `GameData` types already match the real
    // wire format, so we parse directly into those.
    pub async fn get_ongoing_games(&self, n: u8) -> Result<GameDataList, LichessError> {
        let token = self.token.as_deref().ok_or_else(|| {
            LichessError::InvalidRequest("get_ongoing_games requires authentication".into())
        })?;

        let url = format!("{}/account/playing?nb={}", self.base_url, n);
        let response = reqwest::Client::new()
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| LichessError::InvalidRequest(format!("HTTP request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(LichessError::InvalidRequest(format!(
                "account/playing returned {}",
                response.status()
            )));
        }

        let bytes = response.bytes().await.map_err(|e| {
            LichessError::InvalidRequest(format!("failed to read response body: {e}"))
        })?;
        let data: GameDataList = serde_json::from_slice(&bytes).map_err(|e| {
            warn!(
                "Failed to parse GameDataList. Raw: {}",
                String::from_utf8_lossy(&bytes)
            );
            LichessError::Decode {
                context: "GameDataList".into(),
                source: e,
            }
        })?;

        Ok(data)
    }

    // Board
    pub async fn stream_game(
        &self,
        game_id: &str,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<GameStateStreamEvent, LichessError>> + Send>>,
        LichessError,
    > {
        let litchee_stream = self.client.board().stream_game(game_id).await?;
        let mapped_stream = litchee_stream.map(|item| match item {
            Ok(litchess_event) => Ok(map_board_event(&litchess_event)),
            Err(e) => Err(e),
        });
        Ok(Box::pin(mapped_stream))
    }

    pub async fn move_piece(&self, game_id: &str, uci: &str) -> Result<(), LichessError> {
        self.client.board().make_move(game_id, uci, false).await
        // todo!()  Auto deny "offering-draw" (3rd parameter) for now
    }

    pub async fn resign_game(&self, game_id: &str) -> Result<(), LichessError> {
        self.client.board().resign(game_id).await
    }

    pub async fn abort_game(&self, game_id: &str) -> Result<(), LichessError> {
        self.client.board().abort(game_id).await
    }

    // Relations
    /// Usernames of the players the authenticated user follows (Lichess
    /// "friends"). `GET /api/rel/following` streams NDJSON — one user object
    /// per line — which we collect into a plain sorted list for the
    /// FriendlistScreen.
    pub async fn get_following(&self) -> Result<Vec<String>, LichessError> {
        if self.token.is_none() {
            return Err(LichessError::InvalidRequest(
                "get_following requires authentication".into(),
            ));
        }
        let stream = self.client.relations().following().await?;
        tokio::pin!(stream);
        let mut names = Vec::new();
        while let Some(item) = stream.next().await {
            names.push(item?.username);
        }
        names.sort();
        Ok(names)
    }

    // Challenges
    /// Send a friend challenge. Standard variant, random color; the time
    /// control varies with `mode` and the rated flag is forwarded (e06).
    /// Returns the created challenge mapped to our summary type.
    pub async fn send_challenge(
        &self,
        username: &str,
        mode: &ChallengeMode,
        rated: bool,
    ) -> Result<ChallengeSummary, LichessError> {
        if self.token.is_none() {
            return Err(LichessError::InvalidRequest(
                "send_challenge requires authentication".into(),
            ));
        }
        let req = self
            .client
            .challenges()
            .challenge(username)
            .rated(rated)
            .color(LichessChallengeColor::Random);
        let req = match mode {
            ChallengeMode::Unlimited => req,
            // 10:5 real-time default
            ChallengeMode::RealTime => req.clock(600, 5),
            ChallengeMode::Correspondence(days) => req.days(u32::from(*days)),
        };
        let challenge: LichessChallenge = req.send().await?;
        Ok(map_challenge(&challenge, false))
    }

    /// Challenge the Lichess AI (`POST /api/challenge/ai`). The returned game
    /// starts immediately. AI games are always casual on Lichess — no rated
    /// flag is sent.
    ///
    /// NOTE: This bypasses litchee and uses reqwest directly. litchee 0.2.2
    /// parses the response as `LichessGame`, whose `variant` is a string
    /// enum, but the live API returns an object
    /// (`{"key":"standard","name":"Standard","short":"Std"}`) — so the
    /// decode fails (BUG-2026-11-11T122900). Same workaround pattern as
    /// `get_ongoing_games`: raw POST, parse what we need (just the id).
    pub async fn send_challenge_ai(
        &self,
        level: u8,
        mode: &ChallengeMode,
    ) -> Result<AiGame, LichessError> {
        let token = self.token.as_deref().ok_or_else(|| {
            LichessError::InvalidRequest("send_challenge_ai requires authentication".into())
        })?;

        let mut form: Vec<(String, String)> = vec![
            ("level".into(), level.to_string()),
            ("color".into(), "random".into()),
        ];
        match mode {
            ChallengeMode::Unlimited => {}
            // 10:5 real-time default
            ChallengeMode::RealTime => {
                form.push(("clock.limit".into(), "600".into()));
                form.push(("clock.increment".into(), "5".into()));
            }
            ChallengeMode::Correspondence(days) => {
                form.push(("days".into(), days.to_string()));
            }
        }

        let url = format!("{}/challenge/ai", self.base_url);
        let response = reqwest::Client::new()
            .post(&url)
            .bearer_auth(token)
            .form(&form)
            .send()
            .await
            .map_err(|e| LichessError::InvalidRequest(format!("HTTP request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(LichessError::InvalidRequest(format!(
                "challenge/ai returned {}",
                response.status()
            )));
        }

        let bytes = response.bytes().await.map_err(|e| {
            LichessError::InvalidRequest(format!("failed to read response body: {e}"))
        })?;
        let body: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| LichessError::Decode {
                context: "challenge/ai response".into(),
                source: e,
            })?;
        let id = body
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                LichessError::InvalidRequest("challenge/ai response has no game id".into())
            })?
            .to_string();
        Ok(AiGame { id })
    }

    /// Create an open challenge anyone can accept (`POST /api/challenge/open`).
    /// Shows up as outgoing in the Invites screen until accepted.
    pub async fn create_open_challenge(
        &self,
        mode: &ChallengeMode,
        rated: bool,
    ) -> Result<ChallengeSummary, LichessError> {
        if self.token.is_none() {
            return Err(LichessError::InvalidRequest(
                "create_open_challenge requires authentication".into(),
            ));
        }
        let challenges = self.client.challenges();
        let req = challenges.create_open().rated(rated);
        let req = match mode {
            ChallengeMode::Unlimited => req,
            ChallengeMode::RealTime => req.clock(600, 5),
            ChallengeMode::Correspondence(days) => req.days(u32::from(*days)),
        };
        let open: litchee::api::gameplay::challenges::LichessOpenChallenge = req.send().await?;
        Ok(ChallengeSummary {
            id: open.id,
            status: format!("{:?}", open.status).to_lowercase(),
            opponent: String::new(), // open challenge: no opponent until accepted
            incoming: false,
            url: open.url,
            rated: open.rated.unwrap_or(false),
            open: true, // e06s08: mark open challenges so Invites can label them
            rating: None,
            speed: None,
            time_control: None,
        })
    }

    /// List incoming + outgoing challenges, mapped to summaries.
    pub async fn list_challenges(&self) -> Result<Vec<ChallengeSummary>, LichessError> {
        if self.token.is_none() {
            return Err(LichessError::InvalidRequest(
                "list_challenges requires authentication".into(),
            ));
        }
        let list = self.client.challenges().list().await?;
        let mut out = Vec::with_capacity(list.incoming.len() + list.outgoing.len());
        for c in &list.incoming {
            out.push(map_challenge(c, true));
        }
        for c in &list.outgoing {
            out.push(map_challenge(c, false));
        }
        Ok(out)
    }

    pub async fn accept_challenge(&self, challenge_id: &str) -> Result<(), LichessError> {
        self.require_auth("accept_challenge")?;
        self.client.challenges().accept(challenge_id, None).await
    }

    pub async fn decline_challenge(&self, challenge_id: &str) -> Result<(), LichessError> {
        self.require_auth("decline_challenge")?;
        self.client.challenges().decline(challenge_id, None).await
    }

    pub async fn cancel_challenge(&self, challenge_id: &str) -> Result<(), LichessError> {
        self.require_auth("cancel_challenge")?;
        self.client.challenges().cancel(challenge_id, None).await
    }

    fn require_auth(&self, what: &str) -> Result<(), LichessError> {
        if self.token.is_none() {
            return Err(LichessError::InvalidRequest(format!(
                "{what} requires authentication"
            )));
        }
        Ok(())
    }
}

/// Map a litchee challenge to the app's summary. `incoming` is decided by the
/// caller (send_challenge → false; list parsing → side of the list). An
/// outgoing entry with `dest_user: None` is an open challenge (e06s08).
fn map_challenge(c: &LichessChallenge, incoming: bool) -> ChallengeSummary {
    let (user, opponent) = if incoming {
        (
            c.challenger.as_ref(),
            c.challenger.as_ref().map(|u| u.name.clone()),
        )
    } else {
        (
            c.dest_user.as_ref(),
            c.dest_user.as_ref().map(|u| u.name.clone()),
        )
    };
    let opponent = opponent.unwrap_or_default();
    let open = !incoming && c.dest_user.is_none();
    // e07s06: extract rating, speed, time control for detail display.
    let rating = user.and_then(|u| u.rating);
    let speed = c.speed.map(|s| format!("{s:?}").to_lowercase());
    let time_control = c.time_control.as_ref().map(|tc| {
        use litchee::api::gameplay::challenges::LichessTimeControl;
        match tc {
            LichessTimeControl::Clock {
                show,
                limit,
                increment,
            } => show.clone().unwrap_or_else(|| {
                format!("{}+{}", limit.unwrap_or(0) / 60, increment.unwrap_or(0))
            }),
            LichessTimeControl::Correspondence { days_per_turn, .. } => {
                format!("{}d", days_per_turn.unwrap_or(0))
            }
            _ => "∞".to_string(),
        }
    });
    ChallengeSummary {
        id: c.id.clone(),
        status: format!("{:?}", c.status).to_lowercase(),
        opponent,
        incoming,
        url: c.url.clone(),
        rated: c.rated.unwrap_or(false),
        open,
        rating,
        speed,
        time_control,
    }
}

fn map_board_event(lb_event: &LichessBoardEvent) -> GameStateStreamEvent {
    match lb_event {
        LichessBoardEvent::GameFull(full) => {
            let clock: Option<Clock> = full.clock.map(|c| Clock {
                initial: c.initial.unwrap_or(0).max(0) as u64,
                increment: c.increment.unwrap_or(0).max(0) as u64,
            });

            let full_event = GameFullEvent {
                variant: map_variant_info(full.variant.as_ref()),
                speed: Speed::Blitz,
                perf: PerfMode {
                    name: "blitz".into(),
                },
                rated: full.rated.unwrap_or(false),
                created_at: full.created_at.unwrap_or(0).max(0) as u64,
                white: map_player(full.white.as_ref()),
                black: map_player(full.black.as_ref()),
                initial_fen: full
                    .initial_fen
                    .clone()
                    .unwrap_or_else(|| "startpos".to_string()),
                clock,
                state: map_game_state(&full.state),
            };
            GameStateStreamEvent::GameFull(Box::new(full_event))
        }
        LichessBoardEvent::GameState(state) => {
            if is_terminal(&state.status) {
                GameStateStreamEvent::GameOver(GameOverEvent {
                    moves: state.moves.clone(),
                    wtime: state.wtime.max(0) as u64,
                    btime: state.btime.max(0) as u64,
                    winc: state.winc.max(0) as u64,
                    binc: state.binc.max(0) as u64,
                    wdraw: state.wdraw,
                    bdraw: state.bdraw,
                    wtakeback: state.wtakeback,
                    btakeback: state.btakeback,
                    status: status_to_string(&state.status),
                    winner: state.winner.map(color_to_string).unwrap_or_default(),
                })
            } else {
                GameStateStreamEvent::GameState(map_game_state(state))
            }
        }
        LichessBoardEvent::ChatLine(chat) => GameStateStreamEvent::ChatLine(ChatLineEvent {
            username: chat.username.clone(),
            text: chat.text.clone(),
            room: match chat.room {
                litchee::api::gameplay::board::LichessChatRoom::Player => "player".to_string(),
                litchee::api::gameplay::board::LichessChatRoom::Spectator => {
                    "spectator".to_string()
                }
                _ => "player".to_string(),
            },
        }),
        LichessBoardEvent::OpponentGone(gone) => {
            GameStateStreamEvent::OpponentGone(OpponentGoneEvent {
                gone: gone.gone,
                claim_win_in_seconds: gone.claim_win_in_seconds.unwrap_or(0) as u64,
            })
        }
        // LichessBoardEvent is #[non_exhaustive]
        _ => {
            warn!("Unknown LichessBoardEvent variant — dropping");
            GameStateStreamEvent::ChatLine(ChatLineEvent {
                username: String::new(),
                text: String::new(),
                room: String::new(),
            })
        }
    }
}

fn map_variant_info(variant_info: Option<&LichessVariantInfo>) -> GameVariant {
    match variant_info {
        Some(info) => map_variant(info.key),
        None => map_variant(Some(LichessVariantKey::Standard)),
    }
}

fn map_variant(entry: Option<LichessVariantKey>) -> GameVariant {
    match entry {
        Some(LichessVariantKey::Standard) => GameVariant {
            key: "standard".into(),
            name: "Standard".into(),
            short: None,
        },
        Some(LichessVariantKey::Chess960) => GameVariant {
            key: "chess960".into(),
            name: "Chess960".into(),
            short: None,
        },
        Some(LichessVariantKey::Crazyhouse) => GameVariant {
            key: "crazyhouse".into(),
            name: "Crazyhouse".into(),
            short: None,
        },
        Some(LichessVariantKey::Antichess) => GameVariant {
            key: "antichess".into(),
            name: "Antichess".into(),
            short: None,
        },
        Some(LichessVariantKey::Atomic) => GameVariant {
            key: "atomic".into(),
            name: "Atomic".into(),
            short: None,
        },
        Some(LichessVariantKey::Horde) => GameVariant {
            key: "horde".into(),
            name: "Horde".into(),
            short: None,
        },
        Some(LichessVariantKey::KingOfTheHill) => GameVariant {
            key: "kingOfTheHill".into(),
            name: "King of the Hill".into(),
            short: None,
        },
        Some(LichessVariantKey::RacingKings) => GameVariant {
            key: "racingKings".into(),
            name: "Racing Kings".into(),
            short: None,
        },
        Some(LichessVariantKey::ThreeCheck) => GameVariant {
            key: "threeCheck".into(),
            name: "Three Check".into(),
            short: None,
        },
        Some(LichessVariantKey::FromPosition) => GameVariant {
            key: "fromPosition".into(),
            name: "From Position".into(),
            short: None,
        },
        // LichessVariantKey is #[non_exhaustive]; None defaults to Standard
        _ => GameVariant {
            key: "standard".into(),
            name: "Standard".into(),
            short: None,
        },
    }
}

fn map_player(li_player: Option<&LichessGameEventPlayer>) -> PlayedBy {
    match li_player {
        None => PlayedBy::User(PlayedByPlayer {
            id: "".to_string(),
            name: "".to_string(),
            title: None,
            rating: 0,
        }),
        Some(p) => {
            if let Some(level) = p.ai_level {
                PlayedBy::Ai(PlayedByAi {
                    ai_level: Some(level as u8),
                })
            } else {
                PlayedBy::User(PlayedByPlayer {
                    id: p.id.clone().unwrap_or_default(),
                    name: p.name.clone().unwrap_or_default(),
                    title: title_to_string(p.title),
                    rating: p.rating.unwrap_or(0) as u64,
                })
            }
        }
    }
}

fn title_to_string(title: Option<LichessTitle>) -> Option<String> {
    title.map(|t| match t {
        LichessTitle::Gm => String::from("Gm"),
        LichessTitle::Wgm => String::from("Wgm"),
        LichessTitle::Im => String::from("Im"),
        LichessTitle::Wim => String::from("Wim"),
        LichessTitle::Fm => String::from("Fm"),
        LichessTitle::Wfm => String::from("Wfm"),
        LichessTitle::Nm => String::from("Nm"),
        LichessTitle::Cm => String::from("Cm"),
        LichessTitle::Wcm => String::from("Wcm"),
        LichessTitle::Wnm => String::from("Wnm"),
        LichessTitle::Lm => String::from("Lm"),
        LichessTitle::Bot => String::from("Bot"),
        // LichessTitle is #[non_exhaustive]
        _ => String::from("Unknown"),
    })
}

fn status_to_string(status: &LichessGameStatusName) -> String {
    match status {
        LichessGameStatusName::Created => "created".to_string(),
        LichessGameStatusName::Started => "started".to_string(),
        LichessGameStatusName::Mate => "mate".to_string(),
        LichessGameStatusName::Aborted => "aborted".to_string(),
        LichessGameStatusName::Cheat => "cheat".to_string(),
        LichessGameStatusName::Draw => "draw".to_string(),
        LichessGameStatusName::InsufficientMaterialClaim => "insufficientMaterialClaim".to_string(),
        LichessGameStatusName::NoStart => "noStart".to_string(),
        LichessGameStatusName::Outoftime => "outoftime".to_string(),
        LichessGameStatusName::Resign => "resign".to_string(),
        LichessGameStatusName::Stalemate => "stalemate".to_string(),
        LichessGameStatusName::Timeout => "timeout".to_string(),
        LichessGameStatusName::UnknownFinish => "unknownFinish".to_string(),
        LichessGameStatusName::VariantEnd => "variantEnd".to_string(),
        // LichessGameStatusName is #[non_exhaustive]
        _ => "unknown".to_string(),
    }
}

fn is_terminal(status: &LichessGameStatusName) -> bool {
    !matches!(
        status,
        LichessGameStatusName::Created | LichessGameStatusName::Started
    )
}

fn color_to_string(color: LichessColor) -> String {
    match color {
        LichessColor::White => "white".to_string(),
        LichessColor::Black => "black".to_string(),
    }
}

fn map_game_state(li_state: &LichessGameState) -> GameStateEvent {
    GameStateEvent {
        moves: li_state.moves.clone(),
        wtime: li_state.wtime.max(0) as u64,
        btime: li_state.btime.max(0) as u64,
        winc: li_state.winc.max(0) as u64,
        binc: li_state.binc.max(0) as u64,
        wdraw: li_state.wdraw,
        bdraw: li_state.bdraw,
        wtakeback: li_state.wtakeback,
        btakeback: li_state.btakeback,
        status: status_to_string(&li_state.status),
        winner: li_state.winner.map(color_to_string),
    }
}
