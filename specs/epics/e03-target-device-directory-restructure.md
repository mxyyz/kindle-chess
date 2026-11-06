# Epic: Target-Device Directory Restructure

> **Epic ID**: e03-target-device-directory-restructure  
> **Status**: Scoping (scope-work in progress)  
> **Priority**: TBD  
> **Created**: 2026-11-05  
> **Scope doc**: `specs/product/SCOPE_LATEST.yaml`

## Overview

Restructure the project directories on the target device (the Kindle's `/mnt/us`
layout: `/mnt/us/hellokindle/`, `/mnt/us/documents/` scriptlet, KUAL bundle paths).

Touchpoints (from CLAUDE.md / survey): `kindle_chess/.env.release` `ROOT_DIR`
(compile-time via `build.rs`), `flash.sh` / `flash_usb.sh` deploy paths + the
`secrets/token.json` mv-aside dance, `kindle_KUAL/hellokindle/chess_app.sh`,
`scriptlet/kindle-chess.sh`, `kindle_KUAL/extensions/hellokindle/menu.json`.

## Stories (sliced by slice-tasks, WSJF order)

See `epic.yaml` (manifest) + `eNNsYY-tasks.yaml` per story. Order:
e03s01 phase-1 restructure → e03s02 FriendlistScreen → e03s03 PlayScreen+send →
e03s04 InvitesScreen+accept → e03s05 i18n infra+settings → e03s06 all screens →
**e03s07 = carried t07-phase2-uat (from e02) at the END of the epic**.
