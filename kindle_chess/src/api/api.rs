use crate::api::litchee_adapter::LitcheeAdapter;
use crate::api::traits::PuzzleFetcher;
use crate::models::oauth::TokenInfo;
use crate::models::puzzle::{Puzzle, PuzzleParams, PuzzleResponse, PuzzleSolveRound};

use log::warn;

// ~~~~~~~~~~~~~~~~ PUZZLE-ENDPOINTS ~~~~~~~~~~~~~~~~

/// GET /api/puzzle/daily — the daily puzzle. No authentication required. This
/// endpoint includes the puzzle `fen` directly.
pub async fn get_daily_puzzle() -> Result<Puzzle, Box<dyn std::error::Error>> {
    LitcheeAdapter::anonymous()
        .get_daily_puzzle()
        .await
        .map_err(Into::into)
}

/// `get_daily_puzzle` with an injected fetcher (for testing the parse).
pub async fn get_daily_puzzle_with(
    fetcher: &impl PuzzleFetcher,
) -> Result<Puzzle, Box<dyn std::error::Error>> {
    let url = format!("{}/puzzle/daily", env!("LICHESS_API_BASE"));
    let body = fetcher.get(&url).await.map_err(|e| e.to_string())?;
    let response: PuzzleResponse = serde_json::from_str(&body)?;
    Ok(response.puzzle)
}

/// GET /api/puzzle/{id} — a puzzle by id. No authentication required. Unlike
/// `/puzzle/next`, this endpoint includes the `fen` and `lastMove` fields.
pub async fn get_puzzle_by_id(id: &str) -> Result<Puzzle, Box<dyn std::error::Error>> {
    Ok(LitcheeAdapter::anonymous().get_puzzle_by_id(id).await?)
}

/// `get_puzzle_by_id` with an injected fetcher (for testing the parse).
pub async fn get_puzzle_by_id_with(
    fetcher: &impl PuzzleFetcher,
    id: &str,
) -> Result<Puzzle, Box<dyn std::error::Error>> {
    let url = format!("{}/puzzle/{}", env!("LICHESS_API_BASE"), id);
    let body = fetcher.get(&url).await.map_err(|e| e.to_string())?;
    let response: PuzzleResponse = serde_json::from_str(&body)?;
    Ok(response.puzzle)
}

/// GET /api/puzzle/next — a fresh puzzle matching `params`.
///
/// Authentication is *optional*: with a token Lichess tailors the puzzle to the
/// user's rating and skips ones they've solved. But that path needs the
/// `puzzle:read` OAuth scope — tokens issued before that scope was requested
/// get a 403, so a failed authenticated request transparently retries
/// anonymously.
///
/// Note: `/puzzle/next` itself omits the `fen` / `lastMove` fields, so we take
/// only the puzzle id from it and re-fetch the complete record by id.
pub async fn get_next_puzzle(
    token: Option<TokenInfo>,
    params: PuzzleParams,
) -> Result<Puzzle, Box<dyn std::error::Error>> {
    let next_id = match &token {
        Some(t) => {
            let adapter = LitcheeAdapter::new(&t.access_token)?;
            let next_puzzle: Puzzle = match adapter.get_next_puzzle(&params).await {
                Ok(puzzle) => puzzle,
                Err(e) => {
                    warn!("Authenticated puzzle/next failed ({e}); retrying anonymously");
                    LitcheeAdapter::anonymous().get_next_puzzle(&params).await?
                }
            };
            next_puzzle.id
        }
        None => {
            LitcheeAdapter::anonymous()
                .get_next_puzzle(&params)
                .await?
                .id
        }
    };
    get_puzzle_by_id(&next_id).await
}

/// POST /api/puzzle/batch/{angle} — submit a finished puzzle to the signed-in
/// account, so Lichess records the solve (activity stream, rating) and
/// `/puzzle/next` advances past it.
///
/// Returns `Ok(None)` when there is no token (anonymous — nothing to post to)
/// or when the request fails, e.g. a 403 from tokens issued before the
/// `puzzle:write` scope: local play must not break over a failed post, so the
/// error is logged and swallowed. `Ok(Some(rounds))` carries Lichess' recorded
/// outcome (win flag, rating diff).
pub async fn post_puzzle_solution(
    token: Option<TokenInfo>,
    params: PuzzleParams,
    id: &str,
    win: bool,
) -> Result<Option<Vec<PuzzleSolveRound>>, Box<dyn std::error::Error>> {
    let Some(t) = token else {
        return Ok(None);
    };
    let adapter = LitcheeAdapter::new(&t.access_token)?;
    match adapter
        .post_puzzle_solution(params.batch_angle(), id, win, 0)
        .await
    {
        Ok(rounds) => Ok(Some(rounds)),
        Err(e) => {
            warn!("post_puzzle_solution failed ({e}); puzzle stays local-only");
            Ok(None)
        }
    }
}

// ~~~~~~~~~~~~~~~~ HOME-SCREEN PUZZLE SELECTION ~~~~~~~~~~~~~~~~

/// Pick the puzzle to open from the home screen's Puzzle button.
///
/// Normally this is the daily puzzle. But if the signed-in account has already
/// played today's daily, a fresh `/puzzle/next` puzzle is returned instead so
/// the user isn't handed something they just solved.
///
/// The "already played" check needs the `puzzle:read` scope. With no token, or
/// when the check itself errors (missing scope, network), it degrades quietly
/// to the daily puzzle.
pub async fn get_home_puzzle(
    token: Option<TokenInfo>,
    params: PuzzleParams,
) -> Result<Puzzle, Box<dyn std::error::Error>> {
    match &token {
        Some(t) => {
            let adapter = LitcheeAdapter::new(&t.access_token)?;
            match adapter.get_home_puzzle(&params).await {
                Ok(p) => Ok(p),
                Err(e) => {
                    warn!("get_home_puzzle failed ({e}); showing daily puzzle");
                    Ok(LitcheeAdapter::anonymous().get_daily_puzzle().await?)
                }
            }
        }
        None => Ok(LitcheeAdapter::anonymous().get_daily_puzzle().await?),
    }
}
