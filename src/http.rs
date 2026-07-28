//! Shared HTTP client for Discord API reconnaissance.
//!
//! Provides user-agent rotation, configurable timeouts, bounded retries and
//! polite rate limiting (default 1 request/second), plus explicit handling
//! of Discord's `429 Too Many Requests` responses (honors `Retry-After`).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use colored::Colorize;
use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};

/// Rotating pool of common, unremarkable user-agents.
const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
    "Mozilla/5.0 (X11; Linux x86_64; rv:127.0) Gecko/20100101 Firefox/127.0",
];

/// Polite, rate-limited blocking HTTP client.
pub struct HttpClient {
    client: Client,
    /// Minimum interval between outbound requests.
    min_interval: Duration,
    /// Timestamp of the last request (for throttling).
    last_request: Mutex<Option<Instant>>,
    /// Rotating index into [`USER_AGENTS`].
    ua_index: Mutex<usize>,
    /// Number of retries after the initial attempt.
    retries: usize,
}

impl HttpClient {
    /// Build a client with the given timeout (seconds), politeness rate
    /// (requests per second) and retry count.
    pub fn new(timeout_secs: u64, requests_per_second: f64, retries: usize) -> Result<Self> {
        let min_interval = if requests_per_second > 0.0 {
            Duration::from_secs_f64(1.0 / requests_per_second)
        } else {
            Duration::ZERO
        };

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .connect_timeout(Duration::from_secs(timeout_secs.min(10)))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .context("failed to build HTTP client")?;

        Ok(Self {
            client,
            min_interval,
            last_request: Mutex::new(None),
            ua_index: Mutex::new(0),
            retries,
        })
    }

    /// Pick the next user-agent in rotation.
    fn next_user_agent(&self) -> &'static str {
        let mut idx = self.ua_index.lock().expect("ua_index mutex poisoned");
        let ua = USER_AGENTS[*idx % USER_AGENTS.len()];
        *idx += 1;
        ua
    }

    /// Sleep as needed to honor the politeness interval between requests.
    fn throttle(&self) {
        let mut last = self
            .last_request
            .lock()
            .expect("last_request mutex poisoned");
        if let Some(prev) = *last {
            let elapsed = prev.elapsed();
            if elapsed < self.min_interval {
                std::thread::sleep(self.min_interval - elapsed);
            }
        }
        *last = Some(Instant::now());
    }

    /// Parse Discord's `Retry-After` header (seconds) from a 429 response.
    fn retry_after(resp: &Response) -> Option<u64> {
        resp.headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<f64>().ok())
            .map(|s| s.ceil() as u64)
    }

    /// Perform a GET request with throttling, UA rotation, bounded retries
    /// and Discord 429/`Retry-After` handling.
    pub fn get(&self, url: &str) -> Result<Response> {
        self.get_with_headers(url, &[])
    }

    /// GET with additional caller-supplied headers (e.g. bot Authorization).
    /// Header values are never logged.
    pub fn get_with_headers(&self, url: &str, extra_headers: &[(&str, &str)]) -> Result<Response> {
        let mut last_err: Option<anyhow::Error> = None;

        for attempt in 0..=self.retries {
            self.throttle();

            let mut headers = HeaderMap::new();
            headers.insert(USER_AGENT, HeaderValue::from_static(self.next_user_agent()));
            headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

            let mut request = self.client.get(url).headers(headers);
            for (name, value) in extra_headers {
                if let Ok(val) = HeaderValue::from_str(value) {
                    request = request.header(*name, val);
                }
            }

            match request.send() {
                Ok(resp) => {
                    if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
                        && attempt < self.retries
                    {
                        let wait = Self::retry_after(&resp).unwrap_or(5).min(30);
                        eprintln!(
                            "{} Discord rate limit (429) — waiting {wait}s per Retry-After",
                            "[warn]".yellow()
                        );
                        std::thread::sleep(Duration::from_secs(wait));
                        last_err = Some(anyhow::anyhow!("rate limited (429)"));
                        continue;
                    }
                    return Ok(resp);
                }
                Err(e) => {
                    if attempt < self.retries {
                        eprintln!(
                            "{} request to {} failed (attempt {}/{}): {e}",
                            "[warn]".yellow(),
                            url,
                            attempt + 1,
                            self.retries + 1
                        );
                        std::thread::sleep(Duration::from_secs(2_u64.pow(attempt as u32)));
                    }
                    last_err = Some(anyhow::Error::new(e));
                }
            }
        }

        Err(last_err
            .expect("retry loop always records an error")
            .context(format!("GET {url} failed after retries")))
    }
}
