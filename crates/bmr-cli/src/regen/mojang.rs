//! Mojang's version metadata and the sha1-verified vanilla server jar, cached in `<data>/minecraft`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use bmr_fetch::Http;
use serde::Deserialize;
use sha1::{Digest, Sha1};

use crate::ui::{fetch_progress, progress};

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
/// One request's limit: the jar is ~60 MB, which the mirror's per-tile timeout would cut off on a slow line.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(900);

#[derive(Deserialize)]
struct Manifest {
    versions: Vec<ManifestEntry>,
}

#[derive(Debug, Deserialize)]
struct ManifestEntry {
    id: String,
    url: String,
    sha1: String,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct ServerDownload {
    pub url: String,
    pub sha1: String,
}

/// What we need from a version's JSON.
#[derive(Debug, PartialEq)]
pub struct VersionInfo {
    pub java_major: u32,
    pub server: ServerDownload,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionFile {
    downloads: Downloads,
    java_version: JavaVersion,
}

#[derive(Deserialize)]
struct Downloads {
    server: Option<ServerDownload>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JavaVersion {
    major_version: u32,
}

fn find_version(manifest: &[u8], mc: &str) -> Result<ManifestEntry> {
    let m: Manifest = serde_json::from_slice(manifest).context("parsing Mojang's version manifest")?;
    m.versions.into_iter().find(|v| v.id == mc).with_context(|| format!("Mojang lists no Minecraft version {mc}"))
}

fn parse_version(bytes: &[u8], mc: &str) -> Result<VersionInfo> {
    let v: VersionFile = serde_json::from_slice(bytes).with_context(|| format!("parsing {mc}'s version JSON"))?;
    let server = v.downloads.server.with_context(|| format!("Minecraft {mc} has no server download"))?;
    Ok(VersionInfo { java_major: v.java_version.major_version, server })
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    Sha1::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn ensure_sha1(bytes: &[u8], expected: &str, what: &str) -> Result<()> {
    let actual = sha1_hex(bytes);
    ensure!(actual == expected, "{what}: sha1 {actual}, expected {expected}");
    Ok(())
}

fn get(url: &str) -> Result<Vec<u8>> {
    let (base, rel) = url.rsplit_once('/').with_context(|| format!("bad url {url}"))?;
    Http::with_timeout(base, Duration::ZERO, DOWNLOAD_TIMEOUT)?
        .with_progress(fetch_progress())
        .get(rel)?
        .with_context(|| format!("{url}: not found"))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::create_dir_all(path.parent().context("cache path")?)?;
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, bytes).with_context(|| tmp.display().to_string())?;
    std::fs::rename(&tmp, path).with_context(|| path.display().to_string())
}

/// `mc`'s metadata, from the cache or else Mojang.
pub fn version_info(cache: &Path, mc: &str) -> Result<VersionInfo> {
    let file = cache.join(format!("{mc}.json"));
    if let Ok(bytes) = std::fs::read(&file) {
        return parse_version(&bytes, mc);
    }
    let entry = find_version(&get(MANIFEST_URL)?, mc)?;
    let bytes = get(&entry.url)?;
    ensure_sha1(&bytes, &entry.sha1, &entry.url)?;
    let info = parse_version(&bytes, mc)?;
    write_atomic(&file, &bytes)?;
    Ok(info)
}

/// The cached server jar for `mc`, downloaded (and sha1-checked) if missing or corrupt.
pub fn server_jar(cache: &Path, mc: &str, info: &VersionInfo) -> Result<PathBuf> {
    let jar = cache.join(format!("server-{mc}.jar"));
    if std::fs::read(&jar).is_ok_and(|b| sha1_hex(&b) == info.server.sha1) {
        return Ok(jar);
    }
    progress!("      downloading the Minecraft {mc} server ({})", info.server.url);
    let bytes = get(&info.server.url)?;
    ensure_sha1(&bytes, &info.server.sha1, &info.server.url)?;
    write_atomic(&jar, &bytes)?;
    Ok(jar)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{"latest":{"release":"26.3"},"versions":[
        {"id":"26.3","type":"release","url":"https://x/26.3.json","sha1":"aa","complianceLevel":1},
        {"id":"1.21.11","type":"release","url":"https://x/1.21.11.json","sha1":"bb"}]}"#;

    const VERSION: &str = r#"{"id":"26.3","downloads":{
        "client":{"sha1":"c","size":1,"url":"https://x/client.jar"},
        "server":{"sha1":"s","size":2,"url":"https://x/server.jar"}},
        "javaVersion":{"component":"java-runtime-epsilon","majorVersion":25}}"#;

    #[test]
    fn finds_a_version_in_the_manifest() {
        let e = find_version(MANIFEST.as_bytes(), "1.21.11").unwrap();
        assert_eq!((e.url.as_str(), e.sha1.as_str()), ("https://x/1.21.11.json", "bb"));
        assert!(find_version(MANIFEST.as_bytes(), "9.9").unwrap_err().to_string().contains("9.9"));
    }

    #[test]
    fn parses_server_download_and_java_requirement() {
        let v = parse_version(VERSION.as_bytes(), "26.3").unwrap();
        assert_eq!(v.java_major, 25);
        assert_eq!(v.server, ServerDownload { url: "https://x/server.jar".into(), sha1: "s".into() });
    }

    #[test]
    fn version_without_server_is_an_error() {
        let old = r#"{"downloads":{},"javaVersion":{"majorVersion":8}}"#;
        assert!(parse_version(old.as_bytes(), "a1.0").unwrap_err().to_string().contains("no server"));
    }

    #[test]
    fn sha1_matches_known_vector_and_mismatch_fails() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert!(ensure_sha1(b"abc", "a9993e364706816aba3e25717850c26c9cd0d89d", "x").is_ok());
        assert!(ensure_sha1(b"abc", "00", "x").is_err());
    }
}
