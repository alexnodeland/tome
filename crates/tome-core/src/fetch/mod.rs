//! The HTTP client (implementation plan S1-4, ticket P1-008).
//!
//! Everything Tome fetches goes through [`Fetcher`], which owns the crawl
//! etiquette the PRD declares non-negotiable (§ Crawl etiquette):
//!
//! - `robots.txt` obeyed by default, including `Crawl-delay`, cached per
//!   origin; unreachable robots (5xx / network) means **disallow**, absent
//!   robots (4xx) means allow.
//! - Per-host rate limiting: the interval is the stricter of the configured
//!   `rate_limit_rps` and the host's `Crawl-delay`.
//! - Honest `User-Agent` carrying the project URL — being identifiable is
//!   what makes good-faith negotiation with a host possible.
//! - `Retry-After` honoured on 429/503; exponential backoff on other 5xx
//!   and transport errors; 4xx never retried (the request is wrong, not
//!   unlucky).
//! - Conditional GET via `If-None-Match`/`If-Modified-Since` on re-sync.
//! - **Redirects are followed manually**, not by the HTTP library: every
//!   hop must pass the robots check and the rate limiter, and S1-5's SSRF
//!   filter will slot into the same per-hop seam. A library that silently
//!   chases redirects would bypass all three.
//! - Response bodies are read up to a caller-supplied cap and refused
//!   beyond it — "the crawler OOMed on a 4 GB tarball behind a docs URL"
//!   is not a bug class worth having.
//!
//! Sync on purpose: at ≤ 4 requests/second there is nothing for async to
//! win, and plain blocking functions keep the pipeline testable against
//! `tome-testkit`'s fixture server. (The P2 planning sketches say `async
//! fn scrape` — those are illustrative notes, and this decision supersedes
//! them.)

pub mod robots;

use std::collections::HashMap;
use std::io::Read as _;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use url::Url;

use crate::config::FetchConfig;
use crate::error::{Error, Result};
use robots::RobotsPolicy;

/// `Tome/<version> (+<project url>)` — the PRD's format, not overridable.
pub fn user_agent() -> String {
    format!(
        "Tome/{} (+https://github.com/alexnodeland/tome)",
        env!("CARGO_PKG_VERSION")
    )
}

/// Follow at most this many redirects per logical fetch.
const MAX_REDIRECTS: usize = 5;
/// Attempts per URL (1 initial + retries) for retryable failures.
const MAX_ATTEMPTS: u32 = 4;

/// Cache validators from a previous fetch of the same URL.
#[derive(Debug, Clone, Default)]
pub struct Validators {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

/// A successful fetch.
#[derive(Debug)]
pub struct Fetched {
    /// Where the content actually came from, after redirects. The caller
    /// (S1-6 crawl scope, S1-5 SSRF) cares when this differs from what was
    /// asked for.
    pub final_url: Url,
    pub status: u16,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug)]
pub enum FetchOutcome {
    Fetched(Box<Fetched>),
    /// 304 against the supplied validators: the cached copy is current.
    NotModified,
}

pub struct Fetcher {
    agent: ureq::Agent,
    config: FetchConfig,
    /// Robots policy per origin (scheme://host:port).
    robots: Mutex<HashMap<String, RobotsPolicy>>,
    /// Last request completion per host, for the rate limiter.
    last_request: Mutex<HashMap<String, Instant>>,
    /// Base delay for exponential backoff. Configurable so tests retry in
    /// milliseconds; production uses [`Self::new`]'s default.
    backoff_base: Duration,
}

impl Fetcher {
    pub fn new(config: FetchConfig) -> Self {
        Self::with_backoff_base(config, Duration::from_millis(500))
    }

    /// Test seam: identical behaviour, faster failure schedule.
    pub fn with_backoff_base(config: FetchConfig, backoff_base: Duration) -> Self {
        let agent_config = ureq::config::Config::builder()
            .user_agent(user_agent())
            // Manual redirects — see module docs.
            .max_redirects(0)
            // Statuses are data, not errors; retry/relay logic lives here.
            .http_status_as_error(false)
            .timeout_global(Some(config.timeout))
            .build();
        Self {
            agent: agent_config.new_agent(),
            config,
            robots: Mutex::new(HashMap::new()),
            last_request: Mutex::new(HashMap::new()),
            backoff_base,
        }
    }

    /// Fetch a URL politely. `max_body` caps the bytes read; `validators`
    /// makes the request conditional.
    pub fn fetch(
        &self,
        url: &Url,
        max_body: u64,
        validators: Option<&Validators>,
    ) -> Result<FetchOutcome> {
        let mut current = url.clone();
        for _hop in 0..=MAX_REDIRECTS {
            self.check_robots(&current)?;
            // ---- S1-5 SSRF filter slots in here, per hop. ----
            let response = self.request_with_retry(&current, max_body, validators)?;
            match response {
                HopOutcome::Redirect(next) => {
                    tracing::debug!(from = %current, to = %next, "following redirect");
                    current = next;
                }
                HopOutcome::Done(outcome) => return Ok(outcome),
            }
        }
        Err(Error::Fetch {
            message: format!("more than {MAX_REDIRECTS} redirects"),
        })
    }

    // ---- robots -----------------------------------------------------------

    fn check_robots(&self, url: &Url) -> Result<()> {
        if !self.config.respect_robots {
            // Config validation constrains when this can be set; the fetcher
            // trusts it but says so in the log.
            tracing::debug!(host = %host_key(url), "robots.txt check skipped by configuration");
            return Ok(());
        }
        let policy = self.robots_for(url)?;
        if policy.allows(url.path()) {
            Ok(())
        } else {
            Err(Error::BlockedByRobots)
        }
    }

    fn robots_for(&self, url: &Url) -> Result<RobotsPolicy> {
        let origin = origin_key(url);
        if let Some(policy) = lock(&self.robots).get(&origin) {
            return Ok(policy.clone());
        }

        let mut robots_url = url.clone();
        robots_url.set_path("/robots.txt");
        robots_url.set_query(None);
        robots_url.set_fragment(None);

        // The robots fetch obeys the rate limiter but not robots itself
        // (RFC 9309: /robots.txt is implicitly fetchable), and it is not
        // retried as patiently — one attempt tells us what we need.
        self.rate_limit(&robots_url, None);
        let result = self.agent.get(robots_url.as_str()).call();
        self.mark_request_done(&robots_url);

        let policy = match result {
            Ok(mut response) => {
                let status = response.status().as_u16();
                if (200..300).contains(&status) {
                    // Cap far above the RFC's 500 KiB processing bound.
                    match read_body(&mut response, 1024 * 1024) {
                        Ok(bytes) => RobotsPolicy::parse(&String::from_utf8_lossy(&bytes)),
                        Err(_) => RobotsPolicy::disallow_all(),
                    }
                } else if (400..500).contains(&status) {
                    // No robots.txt is a statement: nothing is disallowed.
                    RobotsPolicy::allow_all()
                } else {
                    // 5xx: the RFC's conservative reading. A failing host is
                    // not a host to crawl harder.
                    RobotsPolicy::disallow_all()
                }
            }
            Err(_) => RobotsPolicy::disallow_all(),
        };

        lock(&self.robots).insert(origin, policy.clone());
        Ok(policy)
    }

    // ---- rate limiting ------------------------------------------------------

    /// Sleep until this host may be contacted again. The interval is the
    /// stricter of the configured rate and the host's `Crawl-delay`.
    fn rate_limit(&self, url: &Url, crawl_delay: Option<Duration>) {
        let interval = {
            let configured = Duration::from_secs_f64(1.0 / self.config.rate_limit_rps);
            crawl_delay.map_or(configured, |d| d.max(configured))
        };
        let host = host_key(url);
        let wait = {
            let last = lock(&self.last_request);
            last.get(&host)
                .and_then(|t| (interval).checked_sub(t.elapsed()))
        };
        if let Some(wait) = wait {
            std::thread::sleep(wait);
        }
    }

    fn mark_request_done(&self, url: &Url) {
        lock(&self.last_request).insert(host_key(url), Instant::now());
    }

    fn crawl_delay(&self, url: &Url) -> Option<Duration> {
        if !self.config.respect_robots {
            return None;
        }
        lock(&self.robots)
            .get(&origin_key(url))
            .and_then(RobotsPolicy::crawl_delay)
    }

    // ---- the request loop ---------------------------------------------------

    fn request_with_retry(
        &self,
        url: &Url,
        max_body: u64,
        validators: Option<&Validators>,
    ) -> Result<HopOutcome> {
        let mut last_error: Option<Error> = None;

        for attempt in 0..MAX_ATTEMPTS {
            if attempt > 0 {
                tracing::debug!(%url, attempt, "retrying");
            }
            self.rate_limit(url, self.crawl_delay(url));

            let mut request = self.agent.get(url.as_str());
            if let Some(v) = validators {
                if let Some(etag) = &v.etag {
                    request = request.header("If-None-Match", etag);
                }
                if let Some(lm) = &v.last_modified {
                    request = request.header("If-Modified-Since", lm);
                }
            }

            let result = request.call();
            self.mark_request_done(url);

            let mut response = match result {
                Ok(response) => response,
                Err(e) => {
                    // Transport failure: retryable with backoff.
                    last_error = Some(Error::Fetch {
                        message: format!("request failed: {e}"),
                    });
                    std::thread::sleep(self.backoff(attempt, None));
                    continue;
                }
            };

            let status = response.status().as_u16();
            match status {
                304 => return Ok(HopOutcome::Done(FetchOutcome::NotModified)),
                200..=299 => {
                    let content_type = header(&response, "content-type");
                    let etag = header(&response, "etag");
                    let last_modified = header(&response, "last-modified");
                    let body = read_body(&mut response, max_body)?;
                    return Ok(HopOutcome::Done(FetchOutcome::Fetched(Box::new(Fetched {
                        final_url: url.clone(),
                        status,
                        body,
                        content_type,
                        etag,
                        last_modified,
                    }))));
                }
                301 | 302 | 303 | 307 | 308 => {
                    let location = header(&response, "location").ok_or_else(|| Error::Fetch {
                        message: format!("redirect ({status}) without a Location header"),
                    })?;
                    let next = url.join(&location).map_err(|e| Error::Fetch {
                        message: format!("redirect to an unparseable location: {e}"),
                    })?;
                    return Ok(HopOutcome::Redirect(next));
                }
                429 | 503 => {
                    // Honour Retry-After when present; back off regardless.
                    let retry_after = header(&response, "retry-after")
                        .and_then(|v| v.trim().parse::<u64>().ok())
                        // A host asking for more than 5 minutes is telling
                        // us to come back later; fail the sync instead of
                        // camping on its doorstep.
                        .map(|secs| Duration::from_secs(secs.min(300)));
                    last_error = Some(Error::Http { status });
                    std::thread::sleep(self.backoff(attempt, retry_after));
                }
                500..=599 => {
                    last_error = Some(Error::Http { status });
                    std::thread::sleep(self.backoff(attempt, None));
                }
                // 4xx: the request is wrong, not unlucky. Never retried.
                _ => return Err(Error::Http { status }),
            }
        }

        Err(last_error.unwrap_or(Error::Fetch {
            message: "request failed with no attempts recorded".into(),
        }))
    }

    /// Exponential backoff: `base * 2^attempt`, overridden upward by an
    /// explicit `Retry-After`.
    fn backoff(&self, attempt: u32, retry_after: Option<Duration>) -> Duration {
        let exponential = self.backoff_base.saturating_mul(1u32 << attempt.min(8));
        retry_after.map_or(exponential, |ra| ra.max(exponential))
    }
}

enum HopOutcome {
    Redirect(Url),
    Done(FetchOutcome),
}

fn header(response: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

/// Read at most `max` bytes; more than `max` is an error, not a truncation —
/// a silently truncated page would be stored, hashed, and served as if
/// complete.
fn read_body(response: &mut ureq::http::Response<ureq::Body>, max: u64) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    let mut reader = response.body_mut().as_reader().take(max + 1);
    reader.read_to_end(&mut body).map_err(|e| Error::Fetch {
        message: format!("reading the response failed: {e}"),
    })?;
    if body.len() as u64 > max {
        return Err(Error::TooLarge { limit: max });
    }
    Ok(body)
}

fn host_key(url: &Url) -> String {
    format!(
        "{}:{}",
        url.host_str().unwrap_or_default(),
        url.port_or_known_default().unwrap_or_default()
    )
}

fn origin_key(url: &Url) -> String {
    format!("{}://{}", url.scheme(), host_key(url))
}

/// Survive mutex poisoning: the maps hold politeness bookkeeping, and a
/// panicked sibling thread must not turn every later fetch into a panic.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
