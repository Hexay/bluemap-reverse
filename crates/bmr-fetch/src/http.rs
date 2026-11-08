//! HTTP GET relative to a site root, with retries, politeness delay and magic-byte decompression.

use std::thread;
use std::time::Duration;

use anyhow::{Result, anyhow};
use ureq::Agent;
use ureq::http::Response;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

use crate::progress::{OnProgress, Progress, emit};

const RETRIES: u32 = 4;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
/// Per response, compressed and decompressed: the largest real file is textures.json (tens of MB with mods).
const MAX_BODY: u64 = 256 << 20;
/// Longest `Retry-After` honoured; longer asks are capped (a mirror is resumable, a stall is not).
const MAX_RETRY_AFTER: Duration = Duration::from_secs(60);

pub struct Http {
    agent: Agent,
    base: String,
    delay: Duration,
    progress: Option<OnProgress>,
}

/// Why one attempt failed: worth retrying (after the server's `Retry-After`, if it sent one) or not.
enum Failure {
    Transient(anyhow::Error, Option<Duration>),
    Permanent(anyhow::Error),
}

impl Http {
    pub fn new(base: &str, delay: Duration) -> Result<Self> {
        Self::with_timeout(base, delay, DEFAULT_TIMEOUT)
    }

    /// `timeout` bounds one whole request including its body (a large download needs more than a tile).
    pub fn with_timeout(base: &str, delay: Duration, timeout: Duration) -> Result<Self> {
        let agent = Agent::config_builder()
            .user_agent(concat!("bluemap-reverse/", env!("CARGO_PKG_VERSION")))
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            // OS store: the default (bundled webpki roots) makes SChannel reject cross-signed chains like GTS→GlobalSign.
            .tls_config(
                TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build(),
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
                Err(Failure::Transient(e, wait)) if attempt < RETRIES => {
                    emit(self.progress.as_ref(), Progress::Retry { url: &url, attempt, of: RETRIES, error: &e });
                    thread::sleep(wait.unwrap_or(Duration::from_millis(500 << attempt)));
                }
                Err(Failure::Transient(e, _) | Failure::Permanent(e)) => return Err(e.context(url)),
            }
        }
    }

    fn try_get(&self, url: &str) -> Result<Option<Vec<u8>>, Failure> {
        let resp = self.agent.get(url).header("Accept-Encoding", "gzip").call().map_err(transport)?;
        match resp.status().as_u16() {
            204 | 404 => return Ok(None),
            s @ (408 | 429 | 500..) => return Err(Failure::Transient(anyhow!("HTTP {s}"), retry_after(&resp))),
            s if !(200..300).contains(&s) => return Err(Failure::Permanent(anyhow!("HTTP {s}"))),
            _ => {}
        }
        let body = resp.into_body().with_config().limit(MAX_BODY).read_to_vec().map_err(transport)?;
        // BlueMap's own server re-encodes to gzip for us, but SQL (sql.php) and static hosts may send any storage
        // compression, with or without a matching Content-Encoding: trust the bytes, not the headers.
        bmr_compress::decompress_any(body, MAX_BODY as usize).map(Some).map_err(Failure::Permanent)
    }
}

fn transport(e: ureq::Error) -> Failure {
    match e {
        ureq::Error::BodyExceedsLimit(_) => Failure::Permanent(anyhow!("response larger than {} MiB", MAX_BODY >> 20)),
        e => Failure::Transient(e.into(), None),
    }
}

/// `Retry-After` in seconds (the HTTP-date form is rare for rate limits and falls back to backoff).
fn retry_after<B>(resp: &Response<B>) -> Option<Duration> {
    let secs: u64 = resp.headers().get("retry-after")?.to_str().ok()?.trim().parse().ok()?;
    Some(Duration::from_secs(secs).min(MAX_RETRY_AFTER))
}
