use crate::error::ChessError;
// GitHub Releases client. Used by the in-app updater to learn whether a newer
// binary has been published and where to fetch it from.
//
// We hit the public, anonymous endpoint:
//   GET https://api.github.com/repos/<owner>/<repo>/releases/latest
// which is rate-limited to 60 req/h per IP — far more than the app will ever
// need, since checks are user-initiated.
//
// The release workflow uploads assets per tag:
//   - update.tar.gz            bundle: bin/kindle-hello + assets/* + chess_app.sh + MANIFEST.sha256
//   - update.tar.gz.sha256     sha256sum-format sidecar for the bundle
//   - {ASSET_NAME}             legacy binary-only (kept during transition for 0.4.0 clients)
//   - {SHA_NAME}               legacy sidecar
// These names are the contract between this client and .github/workflows/release.yml.

use log::{info, warn};
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use semver::Version;
use serde::Deserialize;

use crate::version;

pub const OWNER: &str = "mxyyz";
pub const REPO: &str = "kindle-chess";
pub const ASSET_NAME: &str = "kindle-chess-armv7-musl";
pub const SHA_NAME: &str = "kindle-chess-armv7-musl.sha256";
pub const BUNDLE_NAME: &str = "update.tar.gz";
pub const BUNDLE_SHA_NAME: &str = "update.tar.gz.sha256";

#[derive(Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// Resolved info for a release that is strictly newer than the running binary.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current: Version,
    pub latest: Version,
    pub release_name: String,
    pub release_notes: String,
    /// Bundle asset URL (preferred) or legacy binary asset URL (fallback).
    pub asset_url: String,
    pub sha256_url: String,
    pub asset_size: u64,
    /// True when the release ships the update.tar.gz bundle (e05+ clients).
    pub is_bundle: bool,
    /// Bundle-specific fields; same as asset_url/sha256_url/asset_size when
    /// is_bundle, None when falling back to the legacy binary pair.
    pub bundle_url: Option<String>,
    pub bundle_sha256_url: Option<String>,
    pub bundle_size: Option<u64>,
}

/// Hits `releases/latest` and returns the parsed JSON.
pub async fn fetch_latest_release() -> Result<Release, ChessError> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        OWNER, REPO
    );

    let mut headers = HeaderMap::new();
    // GitHub rejects requests without a User-Agent.
    let ua = format!("kindle-hello/{}", version::VERSION);
    headers.insert(USER_AGENT, HeaderValue::from_str(&ua)?);
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    info!("Fetching latest release from {}", url);
    let response = client.get(&url).send().await?;
    if !response.status().is_success() {
        return Err(format!("GitHub API returned {}", response.status()).into());
    }
    let release: Release = response.json().await?;
    info!("Latest release: {}", release.tag_name);
    Ok(release)
}

/// `v0.2.0` / `0.2.0` / `release-0.2.0` → `Version("0.2.0")`. Anything else → None.
pub fn parse_tag_version(tag: &str) -> Option<Version> {
    let trimmed = tag.trim_start_matches('v').trim_start_matches("release-");
    Version::parse(trimmed).ok()
}

/// Returns `Ok(Some(_))` if the latest release is strictly newer AND ships
/// usable assets, `Ok(None)` if the running binary is already at or past the
/// latest tag, and `Err(_)` for everything in between. Looks for the
/// update.tar.gz bundle first (e05+), falls back to the legacy binary pair.
pub async fn check_for_update() -> Result<Option<UpdateInfo>, ChessError> {
    let release = fetch_latest_release().await?;
    resolve_update_info(&release, &version::current())
}

/// Pure resolver: given a parsed release and the running version, decide
/// whether an update is available and which assets to use. Extracted from
/// `check_for_update` so it can be unit-tested without network.
pub fn resolve_update_info(
    release: &Release,
    current: &Version,
) -> Result<Option<UpdateInfo>, ChessError> {
    let latest = match parse_tag_version(&release.tag_name) {
        Some(v) => v,
        None => {
            warn!("Unparseable release tag: {}", release.tag_name);
            return Err(format!("unparseable release tag: {}", release.tag_name).into());
        }
    };

    if latest <= *current {
        info!("Up to date (current={}, latest={})", current, latest);
        return Ok(None);
    }

    let find = |name: &str| release.assets.iter().find(|a| a.name == name);

    // Prefer the bundle pair; fall back to the legacy binary pair.
    let (asset_url, sha256_url, asset_size, is_bundle, bundle_url, bundle_sha256_url, bundle_size) =
        match (find(BUNDLE_NAME), find(BUNDLE_SHA_NAME)) {
            (Some(b), Some(s)) => (
                b.browser_download_url.clone(),
                s.browser_download_url.clone(),
                b.size,
                true,
                Some(b.browser_download_url.clone()),
                Some(s.browser_download_url.clone()),
                Some(b.size),
            ),
            _ => match (find(ASSET_NAME), find(SHA_NAME)) {
                (Some(a), Some(s)) => (
                    a.browser_download_url.clone(),
                    s.browser_download_url.clone(),
                    a.size,
                    false,
                    None,
                    None,
                    None,
                ),
                _ => {
                    warn!(
                        "Release {} is missing required assets ({} / {} or {} / {})",
                        release.tag_name, BUNDLE_NAME, BUNDLE_SHA_NAME, ASSET_NAME, SHA_NAME
                    );
                    return Err(format!(
                        "release {} is missing assets — build may still be in progress",
                        release.tag_name
                    )
                    .into());
                }
            },
        };

    Ok(Some(UpdateInfo {
        current: current.clone(),
        latest,
        release_name: release
            .name
            .clone()
            .unwrap_or_else(|| release.tag_name.clone()),
        release_notes: release.body.clone().unwrap_or_default(),
        asset_url,
        sha256_url,
        asset_size,
        is_bundle,
        bundle_url,
        bundle_sha256_url,
        bundle_size,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str, size: u64) -> ReleaseAsset {
        ReleaseAsset {
            name: name.to_string(),
            browser_download_url: format!("https://example.com/{name}"),
            size,
        }
    }

    fn release_with(tag: &str, assets: Vec<ReleaseAsset>) -> Release {
        Release {
            tag_name: tag.to_string(),
            name: None,
            body: None,
            assets,
        }
    }

    #[test]
    fn prefers_bundle_over_legacy() {
        let rel = release_with(
            "v0.5.0",
            vec![
                asset(BUNDLE_NAME, 6_000_000),
                asset(BUNDLE_SHA_NAME, 64),
                asset(ASSET_NAME, 10_000_000),
                asset(SHA_NAME, 64),
            ],
        );
        let cur = Version::new(0, 4, 0);
        let info = resolve_update_info(&rel, &cur).unwrap().unwrap();
        assert!(info.is_bundle);
        assert_eq!(info.bundle_size, Some(6_000_000));
        assert_eq!(info.asset_url, "https://example.com/update.tar.gz");
    }

    #[test]
    fn falls_back_to_legacy_when_no_bundle() {
        let rel = release_with(
            "v0.5.0",
            vec![asset(ASSET_NAME, 10_000_000), asset(SHA_NAME, 64)],
        );
        let cur = Version::new(0, 4, 0);
        let info = resolve_update_info(&rel, &cur).unwrap().unwrap();
        assert!(!info.is_bundle);
        assert_eq!(info.bundle_url, None);
        assert_eq!(
            info.asset_url,
            "https://example.com/kindle-chess-armv7-musl"
        );
    }

    #[test]
    fn up_to_date_returns_none() {
        let rel = release_with(
            "v0.4.0",
            vec![asset(BUNDLE_NAME, 100), asset(BUNDLE_SHA_NAME, 64)],
        );
        let cur = Version::new(0, 4, 0);
        assert!(resolve_update_info(&rel, &cur).unwrap().is_none());
    }

    #[test]
    fn missing_all_assets_is_err() {
        let rel = release_with("v0.5.0", vec![]);
        let cur = Version::new(0, 4, 0);
        assert!(resolve_update_info(&rel, &cur).is_err());
    }

    #[test]
    fn unparseable_tag_is_err() {
        let rel = release_with("not-a-version", vec![asset(BUNDLE_NAME, 100)]);
        let cur = Version::new(0, 4, 0);
        assert!(resolve_update_info(&rel, &cur).is_err());
    }

    #[test]
    fn parse_tag_version_handles_prefixes() {
        assert_eq!(parse_tag_version("v0.4.0").unwrap(), Version::new(0, 4, 0));
        assert_eq!(parse_tag_version("0.4.0").unwrap(), Version::new(0, 4, 0));
        assert_eq!(
            parse_tag_version("release-0.4.0").unwrap(),
            Version::new(0, 4, 0)
        );
        assert!(parse_tag_version("garbage").is_none());
    }
}
