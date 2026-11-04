# Registering Lichess puzzle plays from the app

*Written 2026-10-24, against Lichess OpenAPI spec v2.0.176, lila `master`, and litchee 0.2.1.*

**Short answer:** there is exactly one write endpoint for puzzles —
`POST /api/puzzle/batch/{angle}` ("Solve multiple puzzles at once. Set puzzles as solved
and update ratings.", OAuth scope `puzzle:write`). Fetching a puzzle (daily, next, batch,
by id) is read-only and never tells Lichess the outcome, which is why a finished daily
puzzle in the app is not registered. litchee 0.2.1 already wraps the endpoint as
`client.puzzles().solve_batch(...)`.

## The endpoint

```
POST /api/puzzle/batch/{angle}?nb=0..50
Authorization: Bearer <token with puzzle:write>
Content-Type: application/json

{ "solutions": [ { "id": "D9GHX", "win": true, "rated": true } ] }
```

Response:

```json
{
  "rounds":  [ { "id": "D9GHX", "win": true, "ratingDiff": 16 } ],
  "glicko":  { "rating": 1801.4, "deviation": 69.12 },
  "puzzles": [ "... fresh batch, only present when nb > 0 ..." ]
}
```

- `solutions` may hold up to 99 entries per request (`PuzzleForm.batch.isValid` rejects
  more), but the common case is one solution per finished puzzle.
- `nb > 0` makes the response carry a fresh puzzle batch (same as
  `GET /api/puzzle/batch/{angle}`), so submit + fetch-next can be one request.
- `angle` only steers the next-batch selection and the theme stored on the round.
  Unknown angles fall back to `mix` (`PuzzleAngle.findOrMix`). The website itself uses
  `mix` for the daily puzzle, so `mix` is correct for both daily and endless training.
- Anonymous calls only increment the puzzle's play counter; recording requires auth.

## litchee mapping (0.2.1, `litchee::api::training::puzzles`, via `LichessClient::puzzles()`)

| Lichess API | litchee method |
|---|---|
| `GET /api/puzzle/daily` (public) | `puzzles().daily()` |
| `GET /api/puzzle/{id}` (public) | `puzzles().get(id)` |
| `GET /api/puzzle/next` (`puzzle:read`) | `puzzles().next(angle: Option<&str>, difficulty: Option<&str>, color: Option<LichessColor>)` |
| `GET /api/puzzle/batch/{angle}` (`puzzle:read`) | `puzzles().batch(angle, nb, difficulty, color)` |
| `POST /api/puzzle/batch/{angle}` (`puzzle:write`) | `puzzles().solve_batch(angle: &str, solutions: &[LichessPuzzleSolution], nb: u32)` |
| `GET /api/puzzle/activity` (`puzzle:read`) | `puzzles().activity(max, before, since)` → NDJSON stream |
| `GET /api/puzzle/dashboard/{days}` (`puzzle:read`) | `puzzles().dashboard(days)` |

Request/response DTOs:

- `LichessPuzzleSolution { id: String, win: bool, rated: Option<bool> }` — construct with
  `LichessPuzzleSolution::new(id, win)`; `rated` is skipped when `None`, and the server
  defaults a missing `rated` to **rated** (`PuzzleForm.batch.Solution`:
  `rated: Rated = Rated.Yes`).
- `LichessPuzzleSolveResponse { puzzles: Vec<LichessPuzzleAndGame>, glicko: Option<Value>, rounds: Vec<LichessPuzzleRound> }`
  — `puzzles` is only populated when `nb > 0`.
- `LichessPuzzleRound { id, win: Option<bool>, rating_diff: Option<i32> }`.

All DTOs are `#[non_exhaustive]`; use the constructors/builders, not struct literals.

## What gets recorded (verified in lila `PuzzleFinisher.apply`)

A **round** is upserted per (user, puzzle). The round is the canonical record:
`GET /api/puzzle/activity` and the puzzle dashboard read from the round collection.

| Attempt | Recorded in activity | Rating change |
|---|---|---|
| First attempt, `rated: true` | yes | yes — Glicko recomputed vs the puzzle's rating; delta in `rounds[].ratingDiff`, new rating in `glicko` |
| First attempt, `rated: false` | yes | no |
| Repeat attempt (round exists) | yes — stored `win` updated (`fixedAt` set on first later win) | no |

## Recommended loop for the app

```
GET  /api/puzzle/next  (or batch/{angle})  → present puzzle
user plays                                 → determine win/loss locally
POST /api/puzzle/batch/{angle}?nb=1        → records the round, updates rating,
                                              returns ratingDiff + next puzzle
```

```rust
use litchee::LichessClient;
use litchee::api::training::puzzles::LichessPuzzleSolution;

let client = LichessClient::builder().token("lip_...").build()?;

let puzzle = client.puzzles().next(Some("mix"), None, None).await?;
// ... user plays puzzle.puzzle ...

let mut sol = LichessPuzzleSolution::new(puzzle.puzzle.id.clone(), won);
sol.rated = Some(true);
let res = client.puzzles().solve_batch("mix", &[sol], 1).await?;
// res.rounds[0].rating_diff, res.glicko, res.puzzles[0] = next puzzle
```

Daily puzzle:

```rust
let daily = client.puzzles().daily().await?;
// ... play daily.puzzle ...
client.puzzles()
    .solve_batch("mix", &[LichessPuzzleSolution::new(daily.puzzle.id.clone(), won)], 0)
    .await?;
```

This creates the same round record the website creates when you solve the daily there,
so it shows up in puzzle activity and the dashboard and counts toward the puzzle rating
like any rated solve.

## Authentication

The token needs `puzzle:read` (next/batch/activity/dashboard) and `puzzle:write`
(solve_batch). With a personal access token: create one at
<https://lichess.org/account/oauth/token> with both scopes. With "Log in with Lichess"
OAuth (PKCE): request the `puzzle:read puzzle:write` scopes — litchee's oauth module
builds the authorization URL and exchanges the code; the app's axum callback server
stays.

## Cheating surface (and why it exists)

The server **trusts the client's `win` flag — no moves are verified.** This is by design:
it is how the official mobile app works (solve offline, batch-report later). Consequences
and mitigations:

- Self-inflating your puzzle rating via the API is technically possible. Don't — it is
  against the ToS and Lichess reverts detected abuse.
- Anti-cheat for the website: puzzles opened via a **direct link** on lichess.org
  (`/training/{id}`) before ever playing them are marked *casual*
  (`PuzzleApi.casual.setCasualIfNotYetPlayed`, 30 min TTL). A casual solve is neither
  recorded nor rated (`PuzzleFinisher.apply` returns early without upserting the round).
  This exists so you can't open a puzzle in a new tab, read the solution, and solve it
  in the training tab.
- The casual flag is only ever set by the website's HTML routes. API-fetched puzzles
  never trigger it — **unless** your user opened the same puzzle by its lichess.org
  direct link within the preceding 30 minutes, in which case the app-submitted solve is
  silently treated as casual. Edge case; worth a footnote in the UI, not a blocker.
- Dubious-player heuristics (`userApi.dubiousPuzzle`) stop cheaters from moving *puzzle*
  ratings; the cheater's own rating still moves, which is why self-inflation "works".
- Rate limits: solves are limited to 400/hour per IP, cost 2 per solution for
  authenticated API (non-mobile) clients — i.e. ~200 rated solves/hour/IP. Batch select
  costs `nb/3` (min 1) against the fetch limiter. The `nb=1` submit-and-fetch loop
  keeps the app well inside these.

## Sources

- Lichess OpenAPI spec v2.0.176: `doc/specs/tags/puzzles/api-puzzle-batch-angle.yaml`
  (GET + POST), schemas `PuzzleBatchSolveRequest/Response`, `PuzzleAndGame`,
  `PuzzleActivity` — <https://github.com/lichess-org/api>
- lila `master`: `app/controllers/Puzzle.scala` (`apiBatchSolve`, `apiBatchSelect`,
  `daily`, `serveShow`), `modules/puzzle/src/main/PuzzleFinisher.scala` (round recording,
  Glicko, `rated`, `casual`), `PuzzleApi.scala` (`casual`, `round.upsert`),
  `PuzzleForm.scala` (`Solution(id, win, rated = Rated.Yes)`), `PuzzleActivity.scala`
  (activity streams the round collection) — <https://github.com/lichess-org/lila>
- litchee 0.2.1: `src/api/training/puzzles.rs` and <https://docs.rs/litchee/latest/litchee/>
- Lichess forum, "Puzzle rating not changing after solved rated puzzles" — confirms
  direct-link puzzles become unrated:
  <https://lichess.org/forum/lichess-feedback/puzzle-rating-not-changing-after-solved-rated-puzzles>
