# Security review — e05-safe-inapp-update (branch e05-safe-inapp-update vs main @ 86a426d)

Scope: e05 code changes — `crate/src/api/github.rs`, `crate/src/api/update.rs`, `crate/src/ui/screens/update.rs`, `kindlichess/chess_app.sh`, `.github/workflows/release.yml`, `.github/workflows/rust.yml`, `flash.sh`, `flash_usb.sh`, `run-dev.sh`.

## Findings

- **Secrets exposure:** none. `secrets/token.json` remains gitignored; flash scripts' token-aside dance uses fixed paths only (`kindlichess/secrets/token.json`). No credentials in diff.
- **Command injection (shell):** `chess_app.sh` trampoline runs `tar -xzf "$BUNDLE_NEW" -C "$STAGING"` and `sha256sum -c`. `$BUNDLE_NEW` is a fixed path derived from `$APP_DIR` (env-overridable only for testing). The bundle content is sha256-verified (sidecar) + MANIFEST-verified before install, and originates from GitHub Releases over HTTPS. No attacker-controlled input reaches a shell sink. **Verdict: safe.**
- **Tar path traversal (defense-in-depth note):** busybox tar 1.34.1 strips leading `/`; `../` entries would be extracted relative to `$STAGING`. Mitigated by the sha256 sidecar + MANIFEST chain — only bundles produced by our own release.yml pass verification. LOW severity, confidence 6 — noted, not blocking. Hardening idea for a future epic: add `--no-same-owner` and a post-extract path check in the trampoline.
- **Path traversal in `verify_manifest` (Rust):** `staging.join(rel_path)` does not canonicalize; a MANIFEST entry with `../` could read outside staging. Same trust chain applies (MANIFEST is inside the sha256-verified bundle). LOW, confidence 6 — noted. Hardening: canonicalize + prefix-check before `fs::read`.
- **SSRF / URL injection:** `update.rs`/`github.rs` fetch URLs from GitHub's API response (`browser_download_url`), not user input. Base URL is hardcoded `api.github.com`. **Safe.**
- **CI workflow injection:** `release.yml` uses `${{ steps.tag.outputs.name }}` from `github.ref` (tag push) or a quoted `workflow_dispatch` input in `gh release` commands — quoted properly. `permissions: contents: write` scoped to the job. **Safe.**
- **Unsafe deserialization:** serde JSON only, all DTOs typed. The bundle MANIFEST is parsed by our own `parse_manifest_line` (hex-length check). **Safe.**
- **OAuth/auth:** unchanged from e04 (PKCE, token on disk gitignored). **Safe.**

## Verdict

No HIGH findings (confidence ≥ 8). Two LOW defense-in-depth notes (tar path traversal, manifest join canonicalization) — both mitigated by the sha256 verification chain; hardening deferred to a future epic. Gate: **PASS**.
