//! HTTP GET relative to a site root, with retries, politeness delay and magic-byte decompression.

use std::io::Read;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

use crate::progress::{OnProgress, Progress, emit};

const RETRIES: u32 = 3;
const MAX_BODY: u64 = 1 << 30;

pub struct Http {
    agent: Agent,
    base: String,
    delay: Duration,
    progress: Option<OnProgress>,
}

impl Http {
    pub fn new(base: &str, delay: Duration) -> Result<Self> {
        let agent = Agent::config_builder()
            .user_agent(concat!("bluemap-reverse/", env!("CARGO_PKG_VERSION")))
            .timeout_global(Some(Duration::from_secs(60)))
            .http_status_as_error(false)
            // OS store: the default (bundled webpki roots) makes SChannel reject cross-signed chains like GTS→GlobalSign.
            .tls_config(
                TlsConfig::builder()
                    .provider(TlsProvider::NativeTls)
                    .root_certs(RootCerts::PlatformVerifier)
                    .build(),
            )
            .build()
            .into();
        Ok(Self { agent, base: format!("{}/", base.trim_end_matches('/')), delay, progress: None })
    }

    /// Reports retries to `progress`.
    pub fn with_progress(mut self, progress: Option<OnProgress>) -> Self {
        self.progress = progress;
        self
    }

    /// `None` for 204/404 (BlueMap's "no tile here"). Body is always returned decompressed.
    pub fn get(&self, rel: &str) -> Result<Option<Vec<u8>>> {
        let url = format!("{}{}", self.base, rel);
        let mut attempt = 0;
        loop {
            attempt += 1;
            if !self.delay.is_zero() {
                thread::sleep(self.delay);
            }
            match self.try_get(&url) {
                Ok(v) => return Ok(v),
                Err(e) if attempt < RETRIES => {
                    emit(self.progress.as_ref(), Progress::Retry { url: &url, attempt, of: RETRIES, error: &e });
                    thread::sleep(Duration::from_millis(500 * attempt as u64));
                }
                Err(e) => return Err(e.context(url)),
            }
        }
    }

    fn try_get(&self, url: &str) -> Result<Option<Vec<u8>>> {
        let resp = self.agent.get(url).header("Accept-Encoding", "gzip").call()?;
        match resp.status().as_u16() {
            204 | 404 => return Ok(None),
            s if !(200..300).contains(&s) => bail!("HTTP {s}"),
            _ => {}
        }
        let body = resp.into_body().with_config().limit(MAX_BODY).read_to_vec()?;
        decompress(body).map(Some)
    }
}

/// Servers disagree on Content-Encoding for stored `.gz` files, so trust the bytes, not the headers.
fn decompress(bytes: Vec<u8>) -> Result<Vec<u8>> {
    match bytes.get(..4) {
        Some([0x1f, 0x8b, ..]) => {
            let mut out = Vec::with_capacity(bytes.len() * 4);
            GzDecoder::new(&bytes[..]).read_to_end(&mut out).context("gunzip")?;
            Ok(out)
        }
        // TODO: zstd/lz4/deflate storage compressions (docs/research/01 §1) — add when a site needs them
        Some([0x28, 0xb5, 0x2f, 0xfd]) => bail!("zstd-compressed response not supported yet"),
        _ => Ok(bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use std::io::Write;

    #[test]
    fn gzip_and_raw() {
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(b"\x01\x07prbm").unwrap();
        assert_eq!(decompress(enc.finish().unwrap()).unwrap(), b"\x01\x07prbm");
        assert_eq!(decompress(b"{}".to_vec()).unwrap(), b"{}");
    }
}
