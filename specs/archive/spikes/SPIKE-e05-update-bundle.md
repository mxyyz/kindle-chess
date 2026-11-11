# Spike: e05 update bundle — tar on device + asset embedding feasibility

Spike branch/worktree: `spike/e05-update-bundle` at `/home/mxy/repos/spike-e05-update-bundle` (throwaway code; this doc is the kept learning).

## Question

1. Does the Kindle (k2) have a working `tar` that can create/extract `.tar.gz` on `/mnt/us` (VFAT)? — needed for update-bundle option B.
2. Can the armv7-musl binary embed the device assets via `include_bytes!` (option C fallback), and what does it cost in binary size? — user is against embedding (config/assets split), but wanted it tested together.
3. What does the device environment look like for the sanity checks (arch, free space, dirs, writability)?

## Result

**Answered** (device run 2026-11-10 16:15 UTC via `spike/probe.sh` on the k2).

## Findings

### Embedding (build-side, answered)

- `include_bytes!` of assets compiles fine for `armv7-unknown-linux-musleabi` (static musl) via `cross` (needs `CROSS_CONTAINER_ENGINE=docker` + `env -u LD_LIBRARY_PATH` in this dev environment; podman's crun is broken here).
- **Gotcha:** a `const` (or plain `static` whose only uses are compile-time-foldable, e.g. `.len()` sums) gets constant-folded and the linker drops the bytes — binary stays small and assets are silently NOT embedded. Must force materialization (e.g. `std::hint::black_box`) or use the bytes in a non-foldable way (`fs::write`, hashing). Verified by byte-searching the binary: without black_box, logo bytes absent; with it, present.
- Size cost ≈ embedded bytes: probe with 3 PNGs + font (1.24 MB) → host release 1.72 MB, armv7-musl release 1.79 MB (base without embed ≈ 0.48 MB). Full device assets (`kindlichess/assets`, 36 PNGs ≈ 512 KB) + font (≈ 860 KB) ≈ +1.4 MB on top of the ~10 MB kindle-hello binary.
- Embedding works technically, but user decision stands against it: embedding config would blur what ships where; bundle (option B) keeps one delivery mechanism.

### Release triggering (planning insight, confirmed while reading config)

- release-please config has a single package keyed at path `crate` — files under repo-root `kindlichess/` (assets, `chess_app.sh`) do NOT trigger a release tag. Any in-app update of those files requires either moving them under `crate/` or adding release-please paths / manual dispatch.

### tar + environment on device (answered 2026-11-10, k2 on LAN)

From `/mnt/us/kindlichess/log/spike.log`:
- `uname -m` = **armv7l** (kernel 3.0.35-lab126, k2).
- `/mnt/us` is **fuse.fsp (rw)**, NOT VFAT — the `update.rs` comment about VFAT is stale for this device.
- `df -k /mnt/us`: 3,188,640 1K-blocks total, **3,170,480 available (1% used)** — storage is rarely binding, but the check stays (cheap, and other devices differ).
- **busybox tar 1.34.1 present and works on /mnt/us**: `tar -czf` of all 36 asset PNGs → 400,861 bytes; `tar -xzf` roundtrip → `TAR_ROUNDTRIP_OK` (sha256 of every file matches). Plain `tar -cf` + `gzip` fallback also OK. `sha256sum` present.
- dirs bin/assets/log/secrets/config all exist; write test `WRITABLE_OK`.
- `spike_embed` ran on-device: embedded 1,237,783 bytes, wrote chess-king.png (1,432 bytes), **sha_match=true**.

## Evidence

- Device log: quoted above (spike.log), full copy in the spike worktree history.
- armv7 build: `target/armv7-unknown-linux-musleabi/release/spike_embed` = 1,787,152 bytes, logo.png header bytes found in binary (python byte search).
- host release control: 479,736 bytes without materialized embed vs 1,716,816 with — proves the const-folding gotcha.
- Build command that works here: `env -u LD_LIBRARY_PATH CROSS_CONTAINER_ENGINE=docker RUSTFLAGS="-C target-feature=+crt-static" cross build --target armv7-unknown-linux-musleabi --release --bin spike_embed`

## Implications for the plan

- Option B (single tar.gz bundle + manifest + one sha256 sidecar) remains the recommendation, contingent on the device tar roundtrip succeeding. If the device lacks a usable tar/gzip, fall back to a bundle of individually-hashed files (option A shape) or a tiny custom unpacker.
- Embedding is viable but rejected on scope grounds; noted the black_box/materialization gotcha in case embedding is ever used for something small.
- The release workflow must upload the bundle asset(s) and the release trigger must cover `kindlichess/` changes (or those files move under `crate/`).
- Sanity checks (arch, dirs, free space ≥ download+install, writability) map directly onto probe outputs: `uname -m`, `df -k /mnt/us`, dir checks — all doable from the app via `df`/`stat` on ROOT_DIR or from the launcher script.

## What was NOT explored

- Script self-update (`chess_app.sh.new` → mv + re-exec) on the device — designed, not yet run on-device.
- Bundle install atomicity on a full/busy /mnt/us beyond the df check.
- Behavior of `mv` over a running script on fuse.fsp (assumed same as the working binary trampoline).

## Recommendation

**Proceed with option B (single tar.gz bundle).** Device tar roundtrip is proven, sha256sum exists, ~3.1 GB free, arch is armv7l. Stage-as-`.new` + sha256 verify (bundle sidecar, then per-file manifest) + preflight (arch/dirs/writable/free space) + trampoline install at next launch. Keep the legacy binary-only assets on the release during the transition so 0.4.0 clients can still update once.
