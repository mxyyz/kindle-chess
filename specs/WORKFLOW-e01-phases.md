# Workflow — e01 Phase 1 finish + Phase 2 execution

> type: workflow-recipe
> context: bigpowers skill sequence for epic e01-refactor-and-tests.
> Composed from the installed bigpowers skill catalog. Each step names one
> skill and why it applies to the real task.

## Part 1 — Finish Phase 1 (integrate the done work)

Phase 1 (t01–t04) is code-complete, audited, and green (71 tests). "Finishing"
= landing it. Status after this session: Phase-1 files committed on
`puzzle-implementation`; the branch still carries unrelated puzzle-feature WIP.

| # | Skill | Applies to | Why |
|---|-------|-----------|-----|
| 1 | audit-code | t01–t04 | ✅ Done — passed after the FEN parser regression fix. |
| 2 | request-review | t01–t04 | ✅ Done — reviewer subagent caught the overflow panic. |
| 3 | commit-message | Phase-1 files | ✅ Done this session — scoped commits, Phase-1 only. |
| 4 | release-branch (solo-local) | — | **Next:** land `puzzle-implementation` → `main` via `land-branch.sh`, AFTER the puzzle-feature WIP is resolved (it still sits uncommitted in the tree). |

## Part 2 — Work through Phase 2 (t05 → t06 → t07)

Phase 2 is a **refactor** (extract screen structs/impls, layout constants), not
a feature. Spine = `plan-refactor`, NOT `develop-tdd` — no new behavior; the
existing 71 tests are the safety net. t06 depends on t05; t07 depends on t05.

| # | Skill | Applies to | Why |
|---|-------|-----------|-----|
| 1 | survey-context | — | Start a new session: re-read state.yaml, confirm Phase-1 landed, baseline green. |
| 2 | assess-impact | t05 | Extracting structs from ~700-line models/ui.rs ripples through imports in app/app.rs, ui/screens.rs, ui/mod.rs. Map blast radius first. |
| 3 | plan-refactor | t05–t07 | Break each extraction into tiny compile-green commits. Canonical "safe incremental refactor" skill. |
| 4 | kickoff-branch | — | Isolated worktree off protected main; verify clean 71-green baseline before edits. |
| 5 | execute-plan | t05 → t06 → t07 | Sequential batch with a checkpoint after each task — required because t06/t07 depend on t05. verify: per task = `cargo build && cargo test`. |
| 6 | verify-work | after each task | UAT gate: build + full test. Refactor must be behavior-preserving; green suite is the proof. |
| 7 | audit-code | before landing | Boy-Scout + file-size: t05 targets "ui.rs ~700 → ~100 lines", t07 "no magic numbers". Confirm no dead code left. |
| 8 | request-review | before landing | Independent second opinion on the new module boundaries. |
| 9 | commit-message → release-branch | — | Land Phase 2. |

## Task acceptance gates (from tasks.yaml)

- t05 extract-screen-structs: models/ui.rs ~100 lines; each screen struct own file; `cargo build` + `cargo test` pass.
- t06 extract-screen-impls: ui/screens.rs ~200 lines; each impl co-located; build + test pass.
- t07 layout-constants: ui/layout.rs with SCREEN_WIDTH/HEIGHT/SQUARE_SIZE etc.; magic numbers replaced; build + test pass.

## Notes / landmines

- Refactor-only phase: do not add tests for untested behavior here — that is
  Phase 3 (t08–t11). Keep the diff behavior-preserving.
- The puzzle-feature WIP (ui/renderer/widgets/board/game/oauth, build.rs) is
  unrelated to e01 and was NOT committed with Phase 1 — resolve it before
  release-branch or it will ride along into main.
- Solo-git profile: integrate via land-branch.sh, not `gh pr create`.
