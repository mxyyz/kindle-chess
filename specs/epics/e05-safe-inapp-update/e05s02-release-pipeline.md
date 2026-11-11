# e05s02 — Release pipeline: update.tar.gz bundle + sidecar

**type:** feat | **risk:** P1 | **bcps:** 5 | **delta:** ADDED

## Context
The release workflow currently uploads only the binary + its sha256 sidecar. e05 adds a bundle asset (`update.tar.gz` containing binary + assets + chess_app.sh + MANIFEST.sha256) so the in-app updater can deliver everything the app ships. Legacy binary-only assets are kept during transition so 0.4.0 clients can still update.

## Requirements

#### ADDED: release.yml builds and uploads update.tar.gz + sidecar
Release assets per tag: `update.tar.gz` { bin/kindle-hello, assets/*, chess_app.sh, MANIFEST.sha256 }, `update.tar.gz.sha256` (sha256sum-format), plus legacy `kindle-chess-armv7-musl{,.sha256}`.

#### ADDED: MANIFEST.sha256 inside the bundle
sha256sum of every file in the bundle except MANIFEST itself, paths relative to bundle root, sorted.

#### ADDED: release-please extra-files on kindlichess/
The crate package in release-please-config.json gains extra-files covering `kindlichess/` so asset/launcher changes trigger a release tag.

#### ADDED: local dry run proves the bundle contract
Same commands as the workflow (minus gh upload) produce a bundle that extracts and passes `sha256sum -c MANIFEST.sha256`.

## Steps
1. release.yml bundle staging + upload step → verify: `grep -q 'update.tar.gz' .github/workflows/release.yml && grep -q 'MANIFEST.sha256' .github/workflows/release.yml`
2. Local dry run (build + tar + extract + verify) → verify: mktemp bundle, sha256sum -c passes
3. release-please-config.json extra-files → verify: `python3 -c "import json; ...assert any('kindlichess' in f for f in extra-files)"`
4. Workflow header documents asset contract → verify: `sed -n '1,12p' release.yml | grep -q assets`

## Verification Script
1. Run the dry-run block locally — bundle extracts and MANIFEST verifies
2. `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"` — valid YAML
3. Push a test tag → release has all 4 assets

## Out of scope
- Removing legacy binary assets (keep during transition)
- Changing release-please versioning behavior

## Risks
- Bundle too large for GitHub release asset limit (2 GB) — current bundle ~10 MB, far below
- tar version differences on CI runner — ubuntu-latest has GNU tar, same flags work
