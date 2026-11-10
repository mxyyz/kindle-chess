use crate::error::ChessError;
// In-app updater v2. Streams the update.tar.gz bundle from a GitHub release
// asset, verifies it against the SHA256 sidecar, extracts to a staging dir,
// and verifies the per-file MANIFEST — then parks the verified bundle at
// `<current_exe>.bundle.new` for the launcher trampoline (chess_app.sh) to
// install atomically at next launch.
//
// We deliberately do NOT rename anything over the running binary. The
// launcher script checks for the `.bundle.new` sidecar at startup and
// installs it while no instance is running.
//
// Preflight (arch / dirs / writable / free space) runs BEFORE any download
// so the user gets a readable failure reason instead of a half-written file.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use log::{info, warn};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::api::github::UpdateInfo;
use crate::version;

/// Downloads the new binary, verifies its SHA256 against the sidecar, and
/// renames it over the running executable. On verification failure the
/// `.new` file is deleted and the original binary is left untouched.
pub async fn apply_update(info: &UpdateInfo) -> Result<(), ChessError> {
    let exe = std::env::current_exe()?;
    let new_path: PathBuf = exe.with_extension("new");

    info!(
        "Updating {} → {} ({} bytes)",
        info.current, info.latest, info.asset_size
    );

    // GitHub serves release assets with the same UA requirement as its API.
    let mut headers = HeaderMap::new();
    let ua = format!("kindle-hello/{}", version::VERSION);
    headers.insert(USER_AGENT, HeaderValue::from_str(&ua)?);
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    // ─── 1. Stream the binary, hashing as we go ──────────────────────────────
    let actual_hash = download_and_hash(&client, &info.asset_url, &new_path).await?;

    // ─── 2. Fetch + parse the .sha256 sidecar ────────────────────────────────
    let expected_hash = fetch_expected_hash(&client, &info.sha256_url).await?;

    // ─── 3. Compare. On mismatch, leave the original binary alone. ───────────
    if actual_hash != expected_hash {
        let _ = fs::remove_file(&new_path).await;
        return Err(format!(
            "SHA256 mismatch: expected {}, got {}",
            expected_hash, actual_hash
        )
        .into());
    }
    info!("SHA256 verified: {}", actual_hash);

    // ─── 4. Make executable and leave at <exe>.new for the launcher. ─────────
    // chess_app.sh moves <exe>.new → <exe> on the next launch, while no
    // instance is running. Setting the exec bit here is a no-op on VFAT but
    // keeps the file usable on ext-style filesystems (dev / test_ui).
    let mut perms = fs::metadata(&new_path).await?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&new_path, perms).await?;

    info!(
        "Update staged at {} — relaunch to install {}",
        new_path.display(),
        info.latest
    );

    Ok(())
}

async fn download_and_hash(
    client: &reqwest::Client,
    url: &str,
    dest: &PathBuf,
) -> Result<String, ChessError> {
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(format!("Asset download returned {}", response.status()).into());
    }

    let mut file = fs::File::create(dest).await?;
    let mut hasher = Sha256::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    file.sync_all().await?;

    Ok(format!("{:x}", hasher.finalize()))
}

async fn fetch_expected_hash(client: &reqwest::Client, url: &str) -> Result<String, ChessError> {
    let body = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    // sha256sum format: "<hex>  <filename>". We only care about the first token.
    match body.split_whitespace().next() {
        Some(hex) => Ok(hex.to_lowercase()),
        None => {
            warn!("sha256 sidecar was empty");
            Err("empty sha256 sidecar".into())
        }
    }
}

// ─── Manifest verify (per-file sha256 inside the bundle) ─────────────────────

/// Parse one MANIFEST.sha256 line: "<hex>  <path>" → (hex, path).
/// Returns None for blank lines or malformed entries.
pub fn parse_manifest_line(line: &str) -> Option<(String, String)> {
    let mut parts = line.splitn(2, char::is_whitespace);
    let hex = parts.next()?.trim().to_lowercase();
    let path = parts
        .next()?
        .trim()
        .trim_start_matches('*')
        .trim()
        .to_string();
    if hex.len() == 64 && !path.is_empty() {
        Some((hex, path))
    } else {
        None
    }
}

/// Verify every file listed in a MANIFEST.sha256 against its extracted content
/// in `staging_dir`. Pure-Rust sha2 — no new deps, no shell-out.
pub fn verify_manifest(staging_dir: &Path, manifest: &str) -> Result<(), ChessError> {
    for line in manifest.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (expected, rel_path) =
            parse_manifest_line(line).ok_or_else(|| format!("bad manifest line: {line}"))?;
        let file_path = staging_dir.join(&rel_path);
        let data = std::fs::read(&file_path)
            .map_err(|e| format!("missing bundle file {rel_path}: {e}"))?;
        let actual = format!("{:x}", Sha256::digest(&data));
        if actual != expected {
            return Err(format!(
                "manifest mismatch for {rel_path}: expected {expected}, got {actual}"
            )
            .into());
        }
    }
    Ok(())
}

// ─── apply_update v2 (bundle flow) ────────────────────────────────────────────

/// Bundle update: preflight → stream bundle to `<exe>.bundle.new` → verify
/// sidecar sha256 → extract to `<exe>.bundle.staging/` → verify MANIFEST →
/// leave staged for the launcher trampoline. On any error, remove the
/// partial files and return Err — the running install is never touched.
pub async fn apply_update_v2(info: &crate::api::github::UpdateInfo) -> Result<(), ChessError> {
    let bundle_size = info.bundle_size.unwrap_or(info.asset_size);
    preflight(bundle_size)?;

    let exe = std::env::current_exe()?;
    let bundle_new: PathBuf = exe.with_extension("bundle.new");
    let staging: PathBuf = exe.with_extension("bundle.staging");

    info!(
        "Updating {} → {} (bundle {} bytes)",
        info.current, info.latest, bundle_size
    );

    let mut headers = HeaderMap::new();
    let ua = format!("kindle-hello/{}", crate::version::VERSION);
    headers.insert(USER_AGENT, HeaderValue::from_str(&ua)?);
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    // 1. Stream the bundle, hashing as we go.
    let actual_hash = download_and_hash(
        &client,
        &info.bundle_url.clone().unwrap_or(info.asset_url.clone()),
        &bundle_new,
    )
    .await?;

    // 2. Fetch + compare the sidecar.
    let expected_hash = fetch_expected_hash(
        &client,
        &info
            .bundle_sha256_url
            .clone()
            .unwrap_or(info.sha256_url.clone()),
    )
    .await?;
    if actual_hash != expected_hash {
        let _ = fs::remove_file(&bundle_new).await;
        return Err(format!("SHA256 mismatch: expected {expected_hash}, got {actual_hash}").into());
    }
    info!("Bundle sidecar verified: {actual_hash}");

    // 3. Extract to staging (device tar via shell — no new Rust deps).
    let _ = fs::remove_dir_all(&staging).await;
    fs::create_dir_all(&staging).await?;
    let tar_status = std::process::Command::new("tar")
        .args(["-xzf"])
        .arg(&bundle_new)
        .arg("-C")
        .arg(&staging)
        .status()?;
    if !tar_status.success() {
        let _ = fs::remove_file(&bundle_new).await;
        let _ = fs::remove_dir_all(&staging).await;
        return Err("bundle extraction failed (tar)".into());
    }

    // 4. Verify the per-file MANIFEST inside the extracted bundle.
    let manifest_path = staging.join("MANIFEST.sha256");
    let manifest = fs::read_to_string(&manifest_path)
        .await
        .map_err(|e| format!("bundle has no MANIFEST.sha256: {e}"))?;
    if let Err(e) = verify_manifest(&staging, &manifest) {
        let _ = fs::remove_file(&bundle_new).await;
        let _ = fs::remove_dir_all(&staging).await;
        return Err(e);
    }
    info!("Bundle MANIFEST verified");

    // 5. Clean staging — the launcher re-extracts from bundle.new at launch.
    let _ = fs::remove_dir_all(&staging).await;

    info!(
        "Bundle staged at {} — relaunch to install {}",
        bundle_new.display(),
        info.latest
    );
    Ok(())
}

// ─── Preflight ────────────────────────────────────────────────────────────────
// Sanity checks that run BEFORE any download. Each returns a human-readable
// reason on failure so UpdateScreen can show it.

/// Result of a successful preflight.
#[derive(Debug, Clone)]
pub struct Preflight {
    pub arch: String,
    pub free_bytes: u64,
}

/// Parse `df -k <path>` output, returning available 1K-blocks.
/// Handles both normal ext4-style and busybox/fuse.fsp header variants:
///   "Filesystem           1K-blocks      Used Available Use% Mounted on"
///   "/dev/fsp              3188640     18160   3170480   1% /mnt/us"
pub fn parse_df_k(output: &str) -> Option<u64> {
    for line in output.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        // Need at least 4 columns; "Available" is column index 3.
        if cols.len() >= 4
            && let Ok(kb) = cols[3].parse::<u64>()
        {
            return Some(kb * 1024);
        }
    }
    None
}

/// Pure decision: given observed values, decide whether the update may proceed.
/// `bundle_size` bytes are needed; we require free >= 2x bundle + 10% margin
/// (download + extracted staging copy).
pub fn preflight_decision(
    arch: &str,
    free_bytes: u64,
    bundle_size: u64,
    dirs_ok: bool,
) -> Result<(), String> {
    if arch != "armv7l" {
        return Err(format!("wrong architecture: {arch} (need armv7l)"));
    }
    if !dirs_ok {
        return Err("app directories missing or not writable".to_string());
    }
    let needed = bundle_size
        .saturating_mul(2)
        .saturating_add(bundle_size / 10);
    if free_bytes < needed {
        return Err(format!(
            "not enough free space: {} bytes available, {} needed",
            free_bytes, needed
        ));
    }
    Ok(())
}

/// Run `df -k` on the app root and return available bytes.
fn free_space_bytes(root: &Path) -> Option<u64> {
    let out = std::process::Command::new("df")
        .args(["-k"])
        .arg(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_df_k(&String::from_utf8_lossy(&out.stdout))
}

/// Runtime architecture via `uname -m`.
fn runtime_arch() -> String {
    std::process::Command::new("uname")
        .arg("-m")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Check that the app root and its subdirectories exist and are writable
/// (temp-file probe in each).
fn dirs_writable(root: &Path) -> bool {
    for sub in ["bin", "assets", "log", "secrets", "config"] {
        let dir = root.join(sub);
        if !dir.is_dir() {
            return false;
        }
        let probe = dir.join(".writable_probe");
        if std::fs::write(&probe, b"").is_err() {
            return false;
        }
        let _ = std::fs::remove_file(&probe);
    }
    true
}

/// Run all preflight checks against the compiled-in ROOT_DIR.
pub fn preflight(bundle_size: u64) -> Result<Preflight, ChessError> {
    let root = Path::new(env!("ROOT_DIR"));
    let arch = runtime_arch();
    let free = free_space_bytes(root).unwrap_or(0);
    let dirs_ok = dirs_writable(root);
    preflight_decision(&arch, free, bundle_size, dirs_ok).map_err(ChessError::from)?;
    Ok(Preflight {
        arch,
        free_bytes: free,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_df_k_normal() {
        let out = "Filesystem           1K-blocks      Used Available Use% Mounted on\n\
                   /dev/sda1             51475068  12345678  36129390  26% /\n";
        assert_eq!(parse_df_k(out), Some(36_129_390 * 1024));
    }

    #[test]
    fn parse_df_k_fuse_fsp() {
        // busybox df on Kindle: device name is long, still 4+ columns
        let out = "Filesystem           1K-blocks      Used Available Use% Mounted on\n\
                   /dev/loop0             3188640     18160   3170480   1% /mnt/us\n";
        assert_eq!(parse_df_k(out), Some(3_170_480 * 1024));
    }

    #[test]
    fn parse_df_k_garbage_returns_none() {
        assert_eq!(parse_df_k("no data here"), None);
        assert_eq!(parse_df_k(""), None);
    }

    #[test]
    fn preflight_decision_allows_good_state() {
        // 3 GB free, 6 MB bundle -> need 13.2 MB, plenty
        assert!(preflight_decision("armv7l", 3_000_000_000, 6_000_000, true).is_ok());
    }

    #[test]
    fn preflight_decision_rejects_wrong_arch() {
        let err = preflight_decision("x86_64", 3_000_000_000, 6_000_000, true).unwrap_err();
        assert!(err.contains("architecture"));
    }

    #[test]
    fn preflight_decision_rejects_low_space() {
        // 10 MB free, 6 MB bundle -> need 13.2 MB
        let err = preflight_decision("armv7l", 10_000_000, 6_000_000, true).unwrap_err();
        assert!(err.contains("free space"));
    }

    #[test]
    fn preflight_decision_rejects_missing_dirs() {
        let err = preflight_decision("armv7l", 3_000_000_000, 6_000_000, false).unwrap_err();
        assert!(err.contains("directories"));
    }

    #[test]
    fn parse_manifest_line_normal() {
        let hex64 = "a".repeat(64);
        let (hex, path) = parse_manifest_line(&format!("{hex64}  ./bin/kindle-hello")).unwrap();
        assert_eq!(hex, hex64);
        assert_eq!(path, "./bin/kindle-hello");
    }

    #[test]
    fn parse_manifest_line_binary_marker() {
        // sha256sum uses "*" prefix for binary mode
        let hex64 = "b".repeat(64);
        let (hex, path) = parse_manifest_line(&format!("{hex64}  *assets/logo.png")).unwrap();
        assert_eq!(hex, hex64);
        assert_eq!(path, "assets/logo.png");
    }

    #[test]
    fn parse_manifest_line_rejects_garbage() {
        assert!(parse_manifest_line("").is_none());
        assert!(parse_manifest_line("tooshort  file").is_none());
        assert!(parse_manifest_line("onlyhex").is_none());
    }

    #[test]
    fn verify_manifest_ok_and_mismatch() {
        let dir = std::env::temp_dir().join(format!("e05test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let content = b"hello world";
        let hash = format!("{:x}", Sha256::digest(content));
        std::fs::write(dir.join("file.txt"), content).unwrap();
        let manifest = format!("{hash}  file.txt\n");
        assert!(verify_manifest(&dir, &manifest).is_ok());
        // Mismatch
        let bad = format!("{}  file.txt\n", "0".repeat(64));
        assert!(verify_manifest(&dir, &bad).is_err());
        // Missing file
        let missing = format!("{hash}  nonexistent.txt\n");
        assert!(verify_manifest(&dir, &missing).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
