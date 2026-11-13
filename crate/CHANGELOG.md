# Changelog

## [0.6.0](https://github.com/mxyyz/kindle-chess/compare/v0.5.0...v0.6.0) (2026-11-13)


### Features

* **e07s01:** reusable ListPicker modal widget + PlayScreen friend picker integration ([55c9a9e](https://github.com/mxyyz/kindle-chess/commit/55c9a9ec6c949142d9545b4c1f21112a540f0180))
* **e07s02:** uniform title bar helper (draw_title_bar/draw_page_title) across 9 screens ([0df58f7](https://github.com/mxyyz/kindle-chess/commit/0df58f7bfd14cf782123f407a9d0a75518752595))
* **e07s04:** conditional pagination — next only when more pages, prev hidden on first ([d411ad4](https://github.com/mxyyz/kindle-chess/commit/d411ad4f520fc616c6f7178d2fab4fee290e07d6))
* **e07s05:** recurring fetch (5s×6 then refresh btn) on Games/Invites + Home puzzle badge ([9294c34](https://github.com/mxyyz/kindle-chess/commit/9294c340fc07a6cdd53cc95f42e3c4fd5d0088cb))
* **e07s06:** per-item details on OngoingGames + Invites rows ([0ba479f](https://github.com/mxyyz/kindle-chess/commit/0ba479f9ffaeff93e386b980f55442a1f47f1244))
* **e07s07:** board file/rank labels (orientation-aware) + piece-move shadow fix ([5ae36a4](https://github.com/mxyyz/kindle-chess/commit/5ae36a4e8616d2cee4080d3316a93b6f1b98d47e))
* **e07s08:** ListPicker tap_to_select mode — friend picker selects on first tap ([df411fe](https://github.com/mxyyz/kindle-chess/commit/df411fe46f6a64e9f191cadcc9de2ac0d9bc9b98))
* **e07s09:** uniform 56px titles on all screens + Play title → New Game/Neue Partie ([d427ce0](https://github.com/mxyyz/kindle-chess/commit/d427ce0eaf284b19ba15029485a90941f84385c3))
* **e07s11:** OngoingGames square cards + Invites check/close icon buttons ([277fe52](https://github.com/mxyyz/kindle-chess/commit/277fe52c126b7773cc71e6faeea4e83c530a51d7))
* **e07s12:** board labels gated by AppConfig.board_labels (default true) ([7dfe7f7](https://github.com/mxyyz/kindle-chess/commit/7dfe7f7ba1fc92e57dc66948d1061db3994b4f82))
* **e07:** switch embedded font from AdwaitaSans-Regular to Ithaca-LVB75 ([5979ac0](https://github.com/mxyyz/kindle-chess/commit/5979ac01eac9ff119735a672e3d648fee489cc4b))
* **e08:** move language/update to home tab, replace top.png with 3-icon chrome button ([f454bb1](https://github.com/mxyyz/kindle-chess/commit/f454bb10a4c26259399756bdf7e1411357711598))
* **e08s01:** GeneralSettings model + persistence, rename config to settings.json ([bcda0e6](https://github.com/mxyyz/kindle-chess/commit/bcda0e6c4a09c15336534aca25ca52e2fd7b37b6))
* **e08s02:** SettingsScreen 2-tab UI with context-aware primary tab ([520a967](https://github.com/mxyyz/kindle-chess/commit/520a96713334a436f6c2eec3375d6caff6dad337))
* **e08s03:** route Game/Puzzle/Home settings through 2-tab SettingsScreen ([b1df701](https://github.com/mxyyz/kindle-chess/commit/b1df7016c5ee6d8788fc59fa7b12eb24855eb1fd))


### Bug Fixes

* **e07:** actually fix inverted rank labels + contrast (round 3) ([5446901](https://github.com/mxyyz/kindle-chess/commit/5446901e446358120e70ca3e9e58cee4d86460bd))
* **e07:** address 4 user-reported issues — poll flicker, daily badge, play redraw, board labels ([4a7d424](https://github.com/mxyyz/kindle-chess/commit/4a7d4243c27ca01d44977d86bcbf78d8c5569eaa))
* **e07:** address reviewer must-fix findings (round 1) ([51d1b8c](https://github.com/mxyyz/kindle-chess/commit/51d1b8c3ee9ea2688e21be63dd13d92ba4c19a48))
* **e07:** address round-2 reviewer must-fix findings ([646a279](https://github.com/mxyyz/kindle-chess/commit/646a2793ad3dec45698a58c5ab42b49797874b1e))
* **e07:** address round-4 should-fix items (offline picker, dead i18n keys, badge doc) ([437f959](https://github.com/mxyyz/kindle-chess/commit/437f9591c6e510c7493cb59b7f80fb0fba729a25))
* **e07:** exclude seconds_left from ongoing-games poll comparison ([20e442b](https://github.com/mxyyz/kindle-chess/commit/20e442bebae08ba658743bf0ca576b59515fd605))
* **e07:** expand rated_row_rect and selector_band_rect by 1px to cover X11 border lines ([39ab03b](https://github.com/mxyyz/kindle-chess/commit/39ab03b8116216f3b51bcf9b2af6d9368d17cfd8))
* **e07:** replace skip_full_clear with dirty-rect partial redraw on Play screen ([5e0f539](https://github.com/mxyyz/kindle-chess/commit/5e0f5394bde347c8e645ab60c4e42cf100f66288))
* **e07:** right-align rank numbers on board's right edge ([84533d6](https://github.com/mxyyz/kindle-chess/commit/84533d656c68ad3da5e02255997c85da74db9146))
* **e07s03:** invites pagination icons no longer overlap action button rows ([873618f](https://github.com/mxyyz/kindle-chess/commit/873618fbf2c146335a9652f8ec420399be24f603))
* **e07s10:** periodic Tick drives polls (no flicker) + Home daily-puzzle badge ([1b0504a](https://github.com/mxyyz/kindle-chess/commit/1b0504a8c340c229e164184f93b5f46e900a632d))
* **e07:** selector mode change redraws only the selector band, not full screen ([03eb45f](https://github.com/mxyyz/kindle-chess/commit/03eb45f9707923da2cdef6d64725d339e9a57b30))
* **e08:** cancel game stream on ChessGameScreen drop (BUG-2026-11-13T014911) ([693cc80](https://github.com/mxyyz/kindle-chess/commit/693cc80115474b5df767f6f17b8c6c098fddb04b))
* **e08:** defer board orientation, fix chrome icon loading ([d894a10](https://github.com/mxyyz/kindle-chess/commit/d894a10b974812e6119dfdf622a7e4ac41f5e7c3))
* **e08:** double angled text sizes, halve invite accept/decline icons ([29b2451](https://github.com/mxyyz/kindle-chess/commit/29b2451d54fe43e568a151152279eee14251b137))
* **e08:** flip draw_text_rotated45 to clockwise rotation ([16e0342](https://github.com/mxyyz/kindle-chess/commit/16e03421d865167a72a944ea8ceed8b948220712))
* **e08:** re-read board_labels from config in on_reveal for game/puzzle screens ([635436b](https://github.com/mxyyz/kindle-chess/commit/635436b17f8acc7ab608129b898091f93ddc8980))
* **e08:** reduce accept/decline icon lift to 30% of height ([e74f869](https://github.com/mxyyz/kindle-chess/commit/e74f8698c651abbfe27ec37e425535a7ed73028f))
* **e08:** reposition angled texts and invite action icons ([c6baa10](https://github.com/mxyyz/kindle-chess/commit/c6baa10d62189e4ccb046934a2df02a1d29aa74e))
* **e08:** skip chrome icon load assertion when ASSETS_DIR absent (CI) ([10488ce](https://github.com/mxyyz/kindle-chess/commit/10488ce0743948b56484c059e10cb2b24a9e2edc))


### Performance Improvements

* **e07s10:** throttle synthetic Tick to 5s (synced with poll interval) ([e763cca](https://github.com/mxyyz/kindle-chess/commit/e763ccac1f086da4ac6ef86a9ae5d40959c31a3b))

## [0.5.0](https://github.com/mxyyz/kindle-chess/compare/v0.4.0...v0.5.0) (2026-11-11)


### Features

* **github:** bundle-first asset lookup with legacy fallback + UpdateInfo.is_bundle (e05s03) ([0d653b4](https://github.com/mxyyz/kindle-chess/commit/0d653b4f2044af7814effaa494edb51dff142430))
* **i18n:** update-bundle UX keys EN/DE (e05s05) ([cf80a4d](https://github.com/mxyyz/kindle-chess/commit/cf80a4d31057639c2f2bcb933e5569163d9e5811))
* **ui:** UpdateScreen bundle-aware UX + i18n (e05s05) ([267cc41](https://github.com/mxyyz/kindle-chess/commit/267cc4130763bc83ea565f00c841f3425f0850e1))
* **update:** bundle update client v2 — preflight, staged download, MANIFEST verify (e05s03) ([a8a6661](https://github.com/mxyyz/kindle-chess/commit/a8a6661181e6f4dbd2dd6d32026eecb97f731c05))

## [0.4.0](https://github.com/mxyyz/kindle-chess/compare/v0.3.0...v0.4.0) (2026-11-10)


### Features

* **ci:** format code (42 diffs, no logic change) ([3d1e798](https://github.com/mxyyz/kindle-chess/commit/3d1e798d159ef92ea818d505966c350986ad4c2b))
* **error:** ChessError (Send+Sync) at api/ boundary (e04s05) ([1394c2c](https://github.com/mxyyz/kindle-chess/commit/1394c2cfce01a35ac2b947dcde42647c4595d378))


### Bug Fixes

* **ci:** clippy -D warnings green + fmt/clippy/cache gates in rust.yml ([ecc9d05](https://github.com/mxyyz/kindle-chess/commit/ecc9d05469f58a2ae22def35c7bfe05fa130874f))
* **renderer:** create_window height 0 -&gt; SCREEN_H_U16 (black screen) ([507350f](https://github.com/mxyyz/kindle-chess/commit/507350f326c2ccec105b790232a5fccc8a22ab34))

## [0.3.0](https://github.com/mxyyz/kindle-chess/compare/v0.2.0...v0.3.0) (2026-11-06)


### Features

* **config:** real on-disk persistence — language + puzzle params now survive restarts ([939118f](https://github.com/mxyyz/kindle-chess/commit/939118f12c5a211108cfe15f4e1536e556efb3c0))
* **e02:** phase 2 — toggleable PC:T status bar over the app + top.png button ([a13d347](https://github.com/mxyyz/kindle-chess/commit/a13d347a1069a24b7be562198a8840072093e602))
* **e02:** t08 custom screensaver — logo.png on DeviceLock ([f9707ac](https://github.com/mxyyz/kindle-chess/commit/f9707acb17c0ac69389a3a43cec23ffc31b3688e))
* **e03s01:** release ROOT_DIR -&gt; /mnt/us/kindlichess/ ([24198d1](https://github.com/mxyyz/kindle-chess/commit/24198d17a24c0a67763b1a4bb641e43c5899f546))
* **e03s01:** rename repo bundle dir kindle_KUAL/hellokindle -&gt; kindlichess ([8ae3c62](https://github.com/mxyyz/kindle-chess/commit/8ae3c6201d29a9d4ea184a648d23e35e9c91b654))
* **e03s02:** FriendlistScreen — paged friends list, FriendSelected bubbles to pusher ([9889cbd](https://github.com/mxyyz/kindle-chess/commit/9889cbd76030836e18cf0e24a5faf3628d67ca07))
* **e03s02:** LitcheeAdapter::get_following + BoardAPI pass-through ([ce81228](https://github.com/mxyyz/kindle-chess/commit/ce8122815acb505f1388c7786c3f92bfd881569f))
* **e03s03/s04:** tap-only days slider + per-section invite pagination ([9c9c6bc](https://github.com/mxyyz/kindle-chess/commit/9c9c6bcf0f7dc89046df41a223b7ea0038ae046f))
* **e03s03:** PlayScreen form + Home 'Demo' -&gt; 'Play' ([2d291cf](https://github.com/mxyyz/kindle-chess/commit/2d291cfb84a67987a46afb2f41cb885f36c5ce55))
* **e03s03:** send_challenge (casual, random color, per-mode time control) ([62da8a0](https://github.com/mxyyz/kindle-chess/commit/62da8a027c27729f6ebbedaafb9968b2a21bc05a))
* **e03s04:** InvitesScreen (poll, accept/decline/cancel) + accept -&gt; live game ([9e352a9](https://github.com/mxyyz/kindle-chess/commit/9e352a921baef9cc74d96845d44d6f5df0023102))
* **e03s04:** list/accept/decline/cancel challenge API + BoardAPI pass-throughs ([b18a7bd](https://github.com/mxyyz/kindle-chess/commit/b18a7bd502c074e7013341f02835d19e9eacbb0b))
* **e03s05:** i18n via fluent-rs — EN/DE locales, runtime language setting ([0c575a9](https://github.com/mxyyz/kindle-chess/commit/0c575a9d2e5199e3d38a8308a21732536f6f1f95))
* **e03s06:** EN/DE strings on all remaining screens ([02e892b](https://github.com/mxyyz/kindle-chess/commit/02e892b00f23446bf642eb89cadbe05ef0a56d20))
* **home:** Quit button on the home screen ([815aaa0](https://github.com/mxyyz/kindle-chess/commit/815aaa048cc344d946613756b926b5af8c152632))
* **slider:** range markers at track ends + width matches mode button group ([4469eb4](https://github.com/mxyyz/kindle-chess/commit/4469eb43df545434f3204c0ab00b42e22b18bb2b))
* **system:** powerd wake watcher + dual-source launcher (e02 t03/t04) ([edab5f7](https://github.com/mxyyz/kindle-chess/commit/edab5f738ee02a020f82d6105635aaeaed456f4d))
* **ui:** AWM-conformant WM_NAME window title (e02 t01) ([cc9ffd0](https://github.com/mxyyz/kindle-chess/commit/cc9ffd02115da0e91bf9e3342024bb29a3a7d955))


### Bug Fixes

* **bug:** correspondence 400 — snap days slider to Lichess-allowed values ([267e816](https://github.com/mxyyz/kindle-chess/commit/267e8168f0b16389b6866ff2976ebf0cbc62eda2))
* **e02:** drop appreg.db script, lock-aware powerd watcher, repo-root scriptlet ([d728bc9](https://github.com/mxyyz/kindle-chess/commit/d728bc959ef43fd20be366637db023afa8e2a1d4))
* **e02:** handle ToggleTopBar in ChessGameScreen, not the app loop ([2283109](https://github.com/mxyyz/kindle-chess/commit/2283109b445fb38d9b477c38918e33793eb1f429))
* **e02:** invalidate top screen on DeviceWake so the wake redraw repaints ([dc898bf](https://github.com/mxyyz/kindle-chess/commit/dc898bfbfd1b2e1247711e39a202cb0f94629ff6))
* **e02:** refetch game state on device wake, shorten powerd debounce to 500ms ([9d4ba79](https://github.com/mxyyz/kindle-chess/commit/9d4ba79a3b9dfca05e7e089fdb1b07a24fb5a8c1))
* **e02:** resilient game-stream reconnect loop (root cause of stale board after wake) ([0dd7245](https://github.com/mxyyz/kindle-chess/commit/0dd724580622dbee9337632fd51f5a6a0278c44a))
* **e03s03/s04:** spinner visibility, mode selection marker, dynamic invite rows, follow:read scope ([632c71a](https://github.com/mxyyz/kindle-chess/commit/632c71a9a9b78ba8fd048a16a50e80a09f005965))
* **e03s06:** localize PuzzleSettingsScreen at render (title, all buttons) ([508ad0a](https://github.com/mxyyz/kindle-chess/commit/508ad0a1860ab59f99bb2553d74a38ca6c5cfd3d))
* **slider:** width 796px — midway between 600 and the 992px mode group ([61ae449](https://github.com/mxyyz/kindle-chess/commit/61ae449e218c2c469081b9533ccfaffd95121c2b))

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
