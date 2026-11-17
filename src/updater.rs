//! Verified release discovery and download for Stick's Windows updater.
//! A newer version is not installed until the running program closes.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

const MAX_EXE_SIZE: u64 = 350 * 1024 * 1024;
const RELEASE_EXE_NAME: &str = "Stick.exe";
const RELEASE_URL_PREFIX: &str = "https://github.com/KaniEGY/Stick/releases/download/";

#[derive(Clone, Debug)]
pub struct ReleaseInfo {
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

fn version_parts(s: &str) -> Result<[u64; 4], String> {
    let value = s.trim().trim_start_matches('v');
    let fields: Vec<&str> = value.split('.').collect();
    if !(2..=4).contains(&fields.len()) {
        return Err(format!("Invalid Stick version: {value}"));
    }
    let mut parts = [0; 4];
    for (i, field) in fields.iter().enumerate() {
        if field.is_empty() || !field.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!("Invalid numeric version: {value}"));
        }
        parts[i] = field.parse::<u64>().map_err(|e| e.to_string())?;
    }
    Ok(parts)
}

pub fn is_newer(latest: &str, installed: &str) -> Result<bool, String> {
    Ok(version_parts(latest)?.cmp(&version_parts(installed)?) == Ordering::Greater)
}

pub fn parse_latest_release(json: &Value, current: &str) -> Result<Option<ReleaseInfo>, String> {
    let tag = json.get("tag_name").and_then(Value::as_str)
        .ok_or("GitHub did not return a release tag.")?;
    let version = tag.trim_start_matches('v');
    if !is_newer(version, current)? {
        return Ok(None);
    }
    let asset = json.get("assets").and_then(Value::as_array)
        .and_then(|v| v.iter().find(|a| a.get("name").and_then(Value::as_str) == Some(RELEASE_EXE_NAME)))
        .ok_or("The newer release has no Stick.exe download.")?;
    let url = asset.get("browser_download_url").and_then(Value::as_str)
        .ok_or("The release download URL is missing.")?;
    if !url.starts_with(RELEASE_URL_PREFIX) || !url.ends_with("/Stick.exe") {
        return Err("The release download is not the official Stick GitHub asset.".into());
    }
    let digest = asset.get("digest").and_then(Value::as_str)
        .and_then(|v| v.strip_prefix("sha256:"))
        .ok_or("The release has no SHA-256 digest; update canceled for safety.")?;
    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("GitHub returned an invalid SHA-256 digest.".into());
    }
    let size = asset.get("size").and_then(Value::as_u64)
        .ok_or("GitHub did not provide the download size.")?;
    if !(1024..=MAX_EXE_SIZE).contains(&size) {
        return Err("The release executable size is invalid or too large.".into());
    }
    Ok(Some(ReleaseInfo { version: version.to_string(), url: url.into(), sha256: digest.to_ascii_lowercase(), size }))
}

pub fn check_online(installed: &str) -> Result<Option<ReleaseInfo>, String> {
    let response = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(25)).build()
        .get("https://api.github.com/repos/KaniEGY/Stick/releases/latest")
        .set("User-Agent", "Stick-Studio-Updater")
        .set("Accept", "application/vnd.github+json")
        .call().map_err(|e| format!("Couldn't reach GitHub: {e}"))?;
    let json: Value = response.into_json()
        .map_err(|e| format!("GitHub returned an unreadable response: {e}"))?;
    parse_latest_release(&json, installed)
}

fn verify_pe_file(bytes: &[u8]) -> bool {
    if bytes.len() < 0x40 || &bytes[0..2] != b"MZ" { return false; }
    let offset = u32::from_le_bytes([bytes[0x3c], bytes[0x3d], bytes[0x3e], bytes[0x3f]]) as usize;
    bytes.get(offset..offset.saturating_add(4)) == Some(&b"PE\0\0"[..])
}

pub fn download_verified(info: &ReleaseInfo, current_exe: &Path) -> Result<PathBuf, String> {
    let response = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(180)).build()
        .get(&info.url)
        .set("User-Agent", "Stick-Studio-Updater")
        .call().map_err(|e| format!("Couldn't download the update: {e}"))?;
    let mut bytes = Vec::new();
    response.into_reader().take(MAX_EXE_SIZE + 1).read_to_end(&mut bytes)
        .map_err(|e| format!("Download interrupted: {e}"))?;
    if bytes.len() as u64 != info.size || !verify_pe_file(&bytes) {
        return Err("The downloaded executable is incomplete or invalid.".into());
    }
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if actual != info.sha256 {
        return Err("SHA-256 did not match. The update was discarded.".into());
    }
    let parent = current_exe.parent().ok_or("Couldn't locate the program folder.")?;
    let staged = parent.join("Stick_new.exe");
    let tmp = parent.join("Stick_new.download");
    let _ = std::fs::remove_file(&tmp);
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&tmp)
            .map_err(|e| format!("Can't write to the Stick install folder: {e}"))?;
        file.write_all(&bytes).map_err(|e| format!("Can't write the update: {e}"))?;
        file.sync_all().map_err(|e| format!("Can't finish the update download: {e}"))?;
    }
    // Windows will reject replacing a locked staging file; never touch the active exe.
    std::fs::rename(&tmp, &staged)
        .or_else(|_| {
            std::fs::remove_file(&staged).ok();
            std::fs::rename(&tmp, &staged)
        })
        .map_err(|e| format!("Can't stage the downloaded update: {e}"))?;
    Ok(staged)
}

/// Spawn a separate updater that waits for this executable to exit, swaps it,
/// rolls back if replacement fails, writes version.txt, and starts Stick again.
/// The arguments are separate process args, not interpolated into PowerShell.
pub fn spawn_replacer(current_exe: &Path, staged: &Path, new_version: &str) -> Result<(), String> {
    let _ = version_parts(new_version)?;
    if !staged.is_file() {
        return Err("The downloaded update file is missing.".into());
    }
    let script_path = std::env::temp_dir().join(format!("StickUpdater-{}.ps1", std::process::id()));
    let script = r#"
param(
    [Parameter(Mandatory=$true)][string]$CurrentExe,
    [Parameter(Mandatory=$true)][string]$StagedExe,
    [Parameter(Mandatory=$true)][string]$Version,
    [Parameter(Mandatory=$true)][int]$ParentPid
)
$ErrorActionPreference = 'Stop'
$dir = Split-Path -Parent $CurrentExe
$backup = Join-Path $dir 'Stick_previous.exe'
$versionPath = Join-Path $dir 'version.txt'
$logPath = Join-Path $dir 'Stick_update_error.log'
$previousVersion = if (Test-Path -LiteralPath $versionPath) { [System.IO.File]::ReadAllText($versionPath) } else { '' }
try {
    for ($attempt = 0; $attempt -lt 100; $attempt++) {
        if (-not (Get-Process -Id $ParentPid -ErrorAction SilentlyContinue)) { break }
        Start-Sleep -Milliseconds 250
    }
    if (Get-Process -Id $ParentPid -ErrorAction SilentlyContinue) { throw 'The previous Stick process did not close.' }
    if (-not (Test-Path -LiteralPath $StagedExe)) { throw 'The staged update is missing.' }
    if (Test-Path -LiteralPath $backup) { Remove-Item -LiteralPath $backup -Force }
    Move-Item -LiteralPath $CurrentExe -Destination $backup -Force
    try {
        Move-Item -LiteralPath $StagedExe -Destination $CurrentExe -Force
        [System.IO.File]::WriteAllText($versionPath, $Version)
        Start-Process -FilePath $CurrentExe -WorkingDirectory $dir
    } catch {
        if (Test-Path -LiteralPath $CurrentExe) { Remove-Item -LiteralPath $CurrentExe -Force }
        Move-Item -LiteralPath $backup -Destination $CurrentExe -Force
        [System.IO.File]::WriteAllText($versionPath, $previousVersion)
        Start-Process -FilePath $CurrentExe -WorkingDirectory $dir
        throw
    }
} catch {
    [System.IO.File]::WriteAllText($logPath, $_.Exception.Message)
}
"#;
    std::fs::write(&script_path, script)
        .map_err(|e| format!("Couldn't prepare the restart helper: {e}"))?;
    std::process::Command::new("powershell.exe")
        .arg("-NoProfile").arg("-NonInteractive").arg("-WindowStyle").arg("Hidden")
        .arg("-ExecutionPolicy").arg("Bypass")
        .arg("-File").arg(&script_path)
        .arg("-CurrentExe").arg(current_exe)
        .arg("-StagedExe").arg(staged)
        .arg("-Version").arg(new_version)
        .arg("-ParentPid").arg(std::process::id().to_string())
        .spawn()
        .map_err(|e| format!("Couldn't start the update restarter: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(tag: &str, digest: &str) -> Value {
        serde_json::json!({
            "tag_name":tag,
            "assets":[{
                "name":"Stick.exe",
                "browser_download_url":format!("https://github.com/KaniEGY/Stick/releases/download/{tag}/Stick.exe"),
                "size":25000000,
                "digest":format!("sha256:{digest}")
            }]
        })
    }
    #[test] fn version_comparison_is_numeric_not_lexicographic() {
        assert!(is_newer("4.1.0.100","4.1.0.99").unwrap());
        assert!(is_newer("v4.1.0.72","4.1.0").unwrap());
        assert!(!is_newer("4.1.0.72","4.1.0.72").unwrap());
        assert!(!is_newer("2.2.71","4.1.0").unwrap());
    }
    #[test] fn matching_release_is_not_downloaded_again() {
        let r=fixture("v4.1.0.72", &"a".repeat(64));
        assert!(parse_latest_release(&r,"4.1.0.72").unwrap().is_none());
    }
    #[test] fn valid_newer_release_has_checksum() {
        let r=fixture("v4.1.0.73", &"b".repeat(64));
        let parsed=parse_latest_release(&r,"4.1.0.72").unwrap().unwrap();
        assert_eq!(parsed.version,"4.1.0.73");
        assert_eq!(parsed.sha256,"b".repeat(64));
    }
    #[test] fn missing_checksum_is_a_real_error_not_no_update() {
        let mut r=fixture("v4.1.0.73",&"c".repeat(64));
        r["assets"][0]["digest"] = Value::Null;
        assert!(parse_latest_release(&r,"4.1.0.72").is_err());
    }
    #[test] fn reject_html_disguised_as_executable() {
        assert!(!verify_pe_file(b"<html>hello</html>"));
        let mut exe=vec![0u8; 256];
        exe[0..2].copy_from_slice(b"MZ");
        exe[0x3c..0x40].copy_from_slice(&(128u32).to_le_bytes());
        exe[128..132].copy_from_slice(b"PE\0\0");
        assert!(verify_pe_file(&exe));
    }
}
