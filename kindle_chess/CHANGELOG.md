# Changelog

## [0.2.0](https://github.com/mxyyz/kindle-chess/compare/v0.1.2...v0.2.0) (2026-11-04)


### Features

* Add puzle screen and sideboard as well as settings screen for normal chessgame and puzzle. Implement partial updates for sidebars. Add logo to homescreen and refine some buttons ([004190f](https://github.com/mxyyz/kindle-chess/commit/004190f4b0f7811b40f63d7a6cfe27c34abb4abf))
* **api:** LitcheeAdapter::post_puzzle_solution via POST /api/puzzle/batch/{angle} ([e758032](https://github.com/mxyyz/kindle-chess/commit/e758032c3c39e04e29a0753defe7bd3e9d77d399))
* **api:** migrate Lichess API layer to litchee adapter ([07a4ff2](https://github.com/mxyyz/kindle-chess/commit/07a4ff26c9ab28d350967f7c0a171663677f6e14))
* **api:** post_puzzle_solution wrapper with anonymous/403 degrade ([11b9ce8](https://github.com/mxyyz/kindle-chess/commit/11b9ce8402d52e69c4beb14d6e6e0b4b5c503008))
* **config:** user-editable app config with puzzle defaults ([daa05a1](https://github.com/mxyyz/kindle-chess/commit/daa05a1cbc3f2a08a4b91f8dda053b340ea5b030))
* **puzzle:** auto-post solved puzzles to Lichess from PuzzleScreen ([5f3d7ca](https://github.com/mxyyz/kindle-chess/commit/5f3d7ca7294efe2e597cbe76c1f6c8a47ff61df6))
* **puzzle:** PuzzleSession retains the puzzle id ([cb03233](https://github.com/mxyyz/kindle-chess/commit/cb032336bcd53d2ee219923c496744ac82660e7e))


### Bug Fixes

* follow-up fix to documentation ([101b567](https://github.com/mxyyz/kindle-chess/commit/101b56724f06d2dae8060cba3866a0eb1ce767c5))
* **puzzle-sidebar:** full repaint on info change so clearing leaves no residue ([9891cd9](https://github.com/mxyyz/kindle-chess/commit/9891cd939e69db0670af4dde69e727c6f063e171))
* **puzzle:** clear solved-puzzle info at Next-press time ([c3ea294](https://github.com/mxyyz/kindle-chess/commit/c3ea2947a0a2f310c39f0279f98fc669c8c4e1b8))

## [0.1.2](https://github.com/yyyxam/kindle-chess/compare/v0.1.1...v0.1.2) (2026-05-08)


### Bug Fixes

* up-to-date-check giving false true ([56503e1](https://github.com/yyyxam/kindle-chess/commit/56503e1662d17a86033a667f6e12aa50c77a4a21))

## [0.1.1](https://github.com/yyyxam/kindle-chess/compare/v0.1.0...v0.1.1) (2026-05-08)


### Bug Fixes

* Unparsable Cargo.toml ([9e0291d](https://github.com/yyyxam/kindle-chess/commit/9e0291d50775c267119e582bb09168688371cf0a))
