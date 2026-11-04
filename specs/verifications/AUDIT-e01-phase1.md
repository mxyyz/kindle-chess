# Audit — e01 Phase 1 (Test Infrastructure)

> Independent audit (not the author's self-review). Date: 2026-10-18.
> Scope: Phase 1 tasks t01–t04 (test dir, mock HTTP, bitboard tests, puzzle
> tests). t02 was re-finished separately; audited here as shipped.

## Verdict: ⚠️ CONCERNS — 2 must-fix test-quality defects before "APPROVED"

The prior `phase1-review.md` is a self-review by the author ("Reviewer: Main
Agent") and overstates readiness. Its "Clippy: ✅ Passes" and "test boundary
conditions" claims do not survive independent checks.

---

## Gate results (re-run, evidence over claims)

| Gate | Claimed | Actual | Note |
|------|---------|--------|------|
| `cargo build` | pass | ✅ pass | |
| `cargo test` | 65 ok | ✅ 61 lib + 7 int + 3 helper, 0 fail | counts shifted after t02 lib/[lib] split; all green |
| `cargo fmt --check` | pass | ✅ pass (exit 0) | |
| `cargo clippy` (default) | pass | ✅ exit 0 (warnings only) | |
| `cargo clippy -- -D warnings --all-targets` | "no warnings" | ✗ exit 101 | overstated; see below |

Clippy under `-D warnings` fails on 4 categories. **Only 1 is Phase 1's:**

- `unused variable: result` ×2 → `bitboard.rs:403`, `:604` — **Phase 1 (t03)**, must fix.
- `large_enum_variant` ×3 (`board_api.rs`, `events.rs`) — pre-existing production code, **out of Phase 1 scope**.
- `module_inception` ×2 (`app/mod.rs`, `lib.rs` `api::api`) — `api::api` naming predates Phase 1; `lib.rs` is t02's crate split. Out of scope.
- `very complex type` ×1 (`oauth.rs:53`) — pre-existing, out of scope.

→ Phase 1 did not introduce the enum/module/type smells, but **did** leave the
two unused-`result` warnings, and the review wrongly reported clippy clean.

---

## Checklist

### Test Coverage — ✗ FAIL (the core of Phase 1)

- ✗ **`fen_parsing_rank_overflow` (bitboard.rs:400) is a fake test.** It binds
  `let result = from_fen("9/8/8/8/8/8/8/8 …")` then asserts **nothing**; the
  body is comments admitting "for now, skip this test." It always passes and
  tests zero behavior (violates T1 Insufficient Tests, worse than an honest
  `#[ignore]` per T4). **The behavior it names genuinely works** — `from_fen`
  rejects overflow at `bitboard.rs:158` (`if file >= 8 { return Err }`) — so
  the fix is one line: `assert!(result.is_err())`. The author abandoned a
  real, passing guarantee and left a false coverage signal.
- ✗ **`uci_castle_blocked_by_piece` (bitboard.rs:600) tests the opposite of
  its name.** Name promises "blocked castle rejected"; body computes an unused
  `result` for the blocked case, then asserts a **clear-path** castle
  *succeeds*. The named behavior (obstruction → failure) is never tested.
  Mysterious/lying name (N1, Fowler "Mysterious Name"). The review's "minor
  issue #3" admits the impl does not validate castling obstruction — a real
  untested gap dressed as a passing test.
- ✓ Other t03/t04 tests assert through public interfaces, isolated, fast (0.00s).
- ✓ New t02 functions (`get_daily_puzzle_with`, `MockFetcher`) have tests.

### Boy Scout Rule — ✗ FAIL

- ✗ Two assertion-free / misnamed tests left behind are dead/misleading code.
- ✗ Dead bindings: `result` at :403 and :604 (clippy-flagged).

### Code Style — ⚠ PARTIAL

- ✓ Functions short, descriptive names elsewhere.
- ✗ C5/C2: `fen_parsing_rank_overflow` is 7 lines of stale "skip this" comments
  around dead code — obsolete comment + commented-intent smell.

### Supply Chain & Security — ✓ PASS

- ✓ No new deps in Phase 1; no secrets in diff; test-only mock touches no auth
  or network. t02's `ReqwestFetcher` is a thin wrapper, no new attack surface.

### Scope — ✓ PASS (Phase 1 proper)

- ✓ t03/t04 changes confined to bitboard.rs / puzzle.rs tests.
- Note: the clippy enum/module/type failures are pre-existing production debt,
  correctly **out of scope** — logged here, not charged to Phase 1.

### Types & Safety — ✓ PASS · Law of Demeter — ✓ PASS · SOLID — ✓ PASS
### CONVENTIONS.md compliance — N/A (no CONVENTIONS.md in repo)

---

## Resolution (fixes applied 2026-10-18)

1. `fen_parsing_rank_overflow`: fixing the test revealed the **parser itself**
   was buggy — the `file >= 8` overflow guard ran only on a piece char, never
   after a digit, so a lone `"9"` rank was silently accepted (`Ok`). Fixed the
   root cause in `from_fen`. **Reviewer caught a regression** in the first
   attempt: moving the check *after* `bb.set(...)` opened a panic path — a 9th
   piece on the top rank calls `square(8,7)==64` and `bit(64)` panics
   (`1u64 << 64`). Final fix guards `file >= 8` *before* placing a piece, plus
   a post-increment `file > 8` catch for lone digit overruns. Added a
   regression test for the piece-overflow case (`"8R/8/..."`). Both digit and
   piece overflow now return `Err` instead of silently accepting or panicking.
2. `uci_castle_blocked_by_piece`: confirmed `apply_uci_move` does no legality
   check (blocked castle returns `Ok`). Renamed to
   `uci_queenside_castle_clear_path`, dropped the dead binding, and noted the
   obstruction-validation gap as follow-up **t16**.

Post-fix gates: 71 tests pass, `cargo fmt --check` clean, `cargo clippy
--all-targets` has no warnings on bitboard.rs (the two unused-`result` warnings
are gone). Pre-existing production clippy debt (enum/module/type) unchanged —
still out of Phase 1 scope.

## Original must-fix list (now addressed)

1. `fen_parsing_rank_overflow`: add `assert!(result.is_err(), …)` and delete the
   stale skip comments. (Behavior already correct — one-line real test.)
2. `uci_castle_blocked_by_piece`: either assert the blocked case fails, or — if
   the impl genuinely can't detect obstruction — rename to
   `uci_queenside_castle_clear_path`, drop the dead `result`, and log the
   obstruction-validation gap as a follow-up task. Do not keep a test whose name
   lies about what it checks.

## Rationalizations caught

- The self-review labeled both defects "minor issues … acceptable for now." They
  are not minor: a test that asserts nothing and a test that lies about its name
  are exactly the hygiene failures this audit exists to catch.
