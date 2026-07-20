//! Self-update: download and install the latest release from GitHub.
//!
//! Flow:
//! 1. Determine the current executable path.
//! 2. Fetch the latest release from the GitHub releases API.
//! 3. Find the `tslay` asset in the release.
//! 4. Download the asset (capped at `MAX_BINARY_SIZE`).
//! 5. Verify the SHA-256 digest from the asset's `digest` field.
//! 6. Atomically replace the running binary (rename current → `.old`,
//!    write new, restore ownership, remove `.old`).
//!
//! Errors surface via `anyhow` and are printed as `tslay: <msg>` by `cli::run`.
use std::fs;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use ureq::Body;
/// GitHub releases API endpoint for the latest release.
const RELEASES_API: &str = "https://api.github.com/repos/umago/task-slayer/releases/latest";

/// Hard cap on downloaded binary size (10 MB).
const MAX_BINARY_SIZE: u64 = 10 << 20;

/// Asset name to find in the release.
const ASSET_NAME: &str = "tslay";

#[derive(serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(serde::Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    digest: String,
}

/// Run the self-update flow.
pub fn run() -> Result<()> {
    let current_exe =
        std::env::current_exe().context("cannot determine current executable path")?;

    println!("Checking for latest release...");

    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .build(),
    );

    let release = fetch_latest_release(&agent)?;

    let asset = find_asset(&release.assets, ASSET_NAME).ok_or_else(|| {
        anyhow::anyhow!(
            "no '{ASSET_NAME}' asset found in release {}",
            release.tag_name
        )
    })?;

    println!("Downloading tslay {}...", release.tag_name);

    let download_agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(120)))
            .build(),
    );
    let data = download_asset(&download_agent, &asset.browser_download_url)?;
    println!("Downloaded {} bytes.", data.len());

    if asset.digest.is_empty() {
        bail!("no digest available — refusing to install unverified binary");
    }
    verify_digest(&data, &asset.digest)?;
    println!("SHA-256 digest verified.");

    replace_binary(&data, &current_exe)?;

    println!(
        "tslay updated successfully ({} → {}).",
        crate::VERSION,
        release.tag_name
    );
    Ok(())
}

/// Fetch and parse the latest release from the GitHub API.
fn fetch_latest_release(agent: &ureq::Agent) -> Result<GitHubRelease> {
    let resp = agent
        .get(RELEASES_API)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", &format!("tslay/{}", crate::VERSION))
        .call()
        .context("failed to fetch latest release")?;

    if !resp.status().is_success() {
        bail!("GitHub API returned HTTP {}", resp.status().as_u16());
    }

    resp.into_body()
        .read_json::<GitHubRelease>()
        .map_err(|e| anyhow::anyhow!("failed to parse release response: {e}"))
}

/// Find an asset by name in a release's asset list.
fn find_asset<'a>(assets: &'a [GitHubAsset], name: &str) -> Option<&'a GitHubAsset> {
    assets.iter().find(|a| a.name == name)
}

/// Download the asset, enforcing the max size cap.
fn download_asset(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>> {
    let resp = agent
        .get(url)
        .header("User-Agent", &format!("tslay/{}", crate::VERSION))
        .call()
        .context("failed to download binary")?;

    if !resp.status().is_success() {
        bail!("download failed with HTTP {}", resp.status().as_u16());
    }

    let mut body: Body = resp.into_body();
    let mut reader = body.with_config().limit(MAX_BINARY_SIZE + 1).reader();
    let mut data = Vec::new();
    reader
        .read_to_end(&mut data)
        .map_err(|e| anyhow::anyhow!("failed to read response body: {e}"))?;

    if data.len() as u64 > MAX_BINARY_SIZE {
        bail!("downloaded binary exceeds max size of {MAX_BINARY_SIZE} bytes");
    }
    Ok(data)
}

/// Verify the SHA-256 digest of the downloaded data against the expected
/// digest. The digest from GitHub is in the format `sha256:hex...`.
fn verify_digest(data: &[u8], expected: &str) -> Result<()> {
    let expected = expected.split_once(':').map_or(expected, |(_, h)| h);

    let mut hasher = Sha256::new();
    hasher.update(data);
    let actual: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    if actual != expected {
        bail!("digest mismatch: expected {expected}, got {actual}");
    }
    Ok(())
}

/// Atomically replace the current binary with the new data.
///
/// Mirrors the Go reference: rename the current binary to `.old` first
/// (avoids ETXTBSY on Linux), write the new file, restore ownership from
/// the backup, then remove the backup. On write failure, restore the old
/// binary.
fn replace_binary(data: &[u8], current_exe: &Path) -> Result<()> {
    let old_exe = current_exe.with_extension("old");

    // Clean up a leftover backup from a previous failed update.
    if old_exe.exists() {
        fs::remove_file(&old_exe)
            .with_context(|| format!("failed to remove stale backup {}", old_exe.display()))?;
    }

    // Rename the current binary out of the way.
    if current_exe.exists() {
        fs::rename(current_exe, &old_exe)
            .with_context(|| format!("failed to rename {}", current_exe.display()))?;
    }

    // Write the new binary.
    if let Err(e) = fs::write(current_exe, data) {
        // Try to restore the old binary.
        if old_exe.exists()
            && let Err(rename_err) = fs::rename(&old_exe, current_exe)
        {
            eprintln!("tslay: CRITICAL — failed to restore backup after write error: {rename_err}");
        }
        return Err(e)
            .with_context(|| format!("failed to write new binary to {}", current_exe.display()));
    }

    // Make it executable.
    fs::set_permissions(current_exe, fs::Permissions::from_mode(0o755))
        .context("failed to set executable permissions on new binary")?;

    // Preserve ownership from the old binary (Linux only).
    if let Ok(meta) = fs::metadata(&old_exe) {
        let uid = meta.uid();
        let gid = meta.gid();
        if let Err(e) = std::os::unix::fs::chown(current_exe, Some(uid), Some(gid)) {
            // Restore old binary on chown failure.
            if old_exe.exists()
                && let Err(rename_err) = fs::rename(&old_exe, current_exe)
            {
                eprintln!(
                    "tslay: CRITICAL — failed to restore backup after chown error: {rename_err}"
                );
            }
            return Err(e).context("failed to chown new binary");
        }
    }

    // Remove the backup.
    let _ = fs::remove_file(&old_exe);
    Ok(())
}
