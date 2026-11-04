# Epic: Native Kindle Integration

> **Epic ID**: e02-native-kindle-integration  
> **Status**: In progress (Phase 1)  
> **Priority**: High  
> **Created**: 2026-11-05  
> **Based on**: `specs/planning-context.yaml` + kindlemodding.org/kindle-dev research

## Overview

Make kindle-chess feel like an actual Kindle app instead of a KUAL-launched
fullscreen hack. Two phases (hybrid approach, confirmed in elaborate-spec):

- **Phase 1 — lifecycle**: the app registers in `appreg.db` so it appears on the
  home screen (k2, where scriptlets work), keeps KUAL as fallback (k1), sets an
  AWM-conformant X11 `WM_NAME` window title, and reacts to device wake
  (`com.lab126.powerd` via lipc) with a full e-ink refresh.
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
- `appreg.db` (`/var/local/appreg.db`, sqlite3) maps `handlerId` → properties
  (`command`, `lipcId`, `supportedOrientation`, `default-chrome-style`, …) and
  `associations` rows with interface `application`; that is how apps become
  launchable from the home screen. KUAL's `com.notmarek.shell_integration.*`
  rows are the working template.
- lipc services (`com.lab126.pillow`, `winmgr`, `chromebar`, `powerd`, …) are
  reachable from any language via the on-device `lipc-*` shell tools — no C SDK
  needed; from Rust we shell out and degrade to no-ops off-device.
- Out of scope by design: the C++/GTK/Meson toolchain, the WAF/`window.kindle`
  track (Mesquite web apps only), KPM packaging, KUAL removal.

## Success Criteria

- [ ] Phase 1 on-device (k2): home-screen icon launches the app fullscreen, no KUAL needed
- [ ] Phase 1 on-device: after sleep/wake the board is cleanly redrawn (powerd resume → full refresh)
- [ ] KUAL menu entry still works on k1 (fallback preserved)
- [ ] `cargo build` + `cargo test` green; AWM title-builder unit tests exist
- [ ] Phase 2: `PC:T` title, top.png sidebar button above Back opens the Kindle top bar (brightness reachable), layout accounts for top-bar height, home button exits to home
- [ ] `run-dev.sh` (Xephyr) unaffected — AWM/lipc calls are no-ops off-device

## Tasks

See `e02-native-kindle-integration/tasks.yaml`. Phase 1 = t01–t04, Phase 2 = t05–t07.
