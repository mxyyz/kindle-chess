# Epic: Native Kindle Integration

> **Epic ID**: e02-native-kindle-integration  
> **Status**: In progress (Phase 1)  
> **Priority**: High  
> **Created**: 2026-11-05  
> **Based on**: `specs/planning-context.yaml` + kindlemodding.org/kindle-dev research

## Overview

Make kindle-chess feel like an actual Kindle app instead of a KUAL-launched
fullscreen hack. Two phases (hybrid approach, confirmed in elaborate-spec):

- **Phase 1 — lifecycle**: a scriptlet launcher at repo-root `scriptlet/` is deployed
  to `/mnt/us/documents` by the flash scripts, where k2's SH_Integration auto-registers
  it (appreg.db registration was tried for k1 but does not work there — dropped for
  now). KUAL stays as the fallback launcher. The X11 window gets an AWM-conformant
  `WM_NAME` title, and a display-lock-aware powerd watcher (lock + wake via lipc)
  redraws the panel cleanly on unlock; the lock event is the hook for a custom
  screensaver (t08).
- **Phase 2 — chrome**: window title switches to the `PC:T` variant so the Kindle
  top bar is available; a dedicated sidebar button (`assets/top.png`, above the
  Back button) opens the top bar so the user can reach brightness settings;
  pillow handoff reworked so AWM composites; layout reserves top-bar height.

## Background

Research (kindlemodding.org/kindle-dev + wafs-and-mesquite, extracts in session
scratchpad `kindle-dev/`):

- The Kindle's **Awesome Window Manager** reads each window's role from its X11
  title (`WM_NAME`): `L:A_N:application_PC:TS_ID:com.lab126.store` =
  layer/role/persistent-chrome/id. GTK's `gtk_window_set_title` is just a
  `WM_NAME` write — our x11rb renderer can do the same with `change_property`.
- `appreg.db` (`/var/local/appreg.db`, sqlite3) is **Amazon's native** app registry
  (Amazon's own booklets live there). The `com.notmarek.shell_integration.*` rows
  are injected by the jailbreak's Universal Hotfix; on k2 that integration
  auto-indexes any `.sh` in `/mnt/us/documents` as a launchable scriptlet
  (scriptlets.html) — there we deploy `scriptlet/kindle-chess.sh` via the flash
  scripts. A hand-written appreg.db handlerId was tried for k1 but did not make the
  app launchable (possibly account-dependent), so it is dropped for now; KUAL's
  menu.json remains the fallback launcher on k1.
- lipc services (`com.lab126.pillow`, `winmgr`, `chromebar`, `powerd`, …) are
  reachable from any language via the on-device `lipc-*` shell tools — no C SDK
  needed; from Rust we shell out and degrade to no-ops off-device.
- Out of scope by design: the C++/GTK/Meson toolchain, the WAF/`window.kindle`
  track (Mesquite web apps only), KPM packaging, KUAL removal.

## Success Criteria

- [ ] Phase 1 on-device k2: "Kindli-Chess" scriptlet in the library launches the app fullscreen, no KUAL needed
- [ ] Phase 1 on-device k1: KUAL ChessApp entry launches the app (fallback)
- [ ] Phase 1 on-device: lock/unlock observed in logs; board cleanly redrawn on wake
- [ ] KUAL menu entry still works on k1 (fallback preserved)
- [ ] `cargo build` + `cargo test` green; AWM title-builder unit tests exist
- [ ] Phase 2: `PC:T` title, top.png sidebar button above Back opens the Kindle top bar (brightness reachable), layout accounts for top-bar height, home button exits to home
- [ ] `run-dev.sh` (Xephyr) unaffected — AWM/lipc calls are no-ops off-device

## Tasks

See `e02-native-kindle-integration/tasks.yaml`. Phase 1 = t01–t04, Phase 2 = t05–t07.
