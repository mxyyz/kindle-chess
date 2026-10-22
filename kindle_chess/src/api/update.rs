// In-app updater. Streams a new binary from a GitHub release asset, verifies
// it against the SHA256 sidecar, and parks the verified file at
// `<current_exe>.new`.
//
// We deliberately do NOT rename it over the running binary. /mnt/us on Kindle
// is VFAT, where replacing a busy executable in-place is fragile — VFAT lacks
// the inode-based open-file semantics that make the trick work cleanly on
// ext4. Instead, the KUAL launcher script (`chess_app.sh`) checks for the
// `.new` sidecar at startup and renames it into place *before* exec-ing — so
// the swap always happens while no instance is running.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use futures::StreamExt;
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
pub async fn apply_update(info: &UpdateInfo) -> Result<(), Box<dyn std::error::Error>> {
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
) -> Result<String, Box<dyn std::error::Error>> {
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

async fn fetch_expected_hash(
    client: &reqwest::Client,
    url: &str,
) -> Result<String, Box<dyn std::error::Error>> {
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
