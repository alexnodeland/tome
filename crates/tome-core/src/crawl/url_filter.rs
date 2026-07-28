//! URL scoping for a crawl (implementation plan S1-6, ticket P1-011).
//!
//! Decides whether one discovered URL is in scope. Three gates, in order:
//!
//! 1. **Scheme** — http(s) only. `mailto:`, `javascript:`, `data:` are not
//!    crawlable (the parser already drops them, but a filter must not assume
//!    its input was pre-cleaned).
//! 2. **Host scope** — same *registrable domain* as the entry point, so a
//!    crawl of `docs.python.org` follows `docs.python.org/3/...` and
//!    `www.python.org` but never wanders onto a link to `github.com`. The
//!    PRD's default is "same registrable domain … explicit opt-in per host".
//! 3. **Include / exclude patterns** — regex against the URL path, **exclude
//!    wins**. This is where a config narrows a crawl (`include: ^/3/`) or
//!    carves out a subtree (`exclude: /_sources/`).
//!
//! The nodejs.org case from SPIKE-010 lives here: its `robots.txt` allows
//! `/api/` but disallows `/docs/`. Robots is enforced by the fetcher, but a
//! source config for Node would *also* set `include: ^/api/` so the crawl
//! never queues the disallowed tree in the first place — cheaper than
//! fetching each `/docs/` URL only to have robots reject it.

use regex::Regex;
use url::Url;

/// Whether a URL is in scope for a crawl, and why not when it isn't.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    InScope,
    WrongScheme,
    OffHost,
    Excluded,
    NotIncluded,
}

impl Scope {
    pub fn is_in_scope(self) -> bool {
        matches!(self, Scope::InScope)
    }
}

/// The scope test for one crawl. Built from a [`SourceConfig`]'s generic
/// scraper settings plus the entry URL; the regexes are already compiled by
/// the config parser (S1-3), so this never compiles at match time.
#[derive(Debug, Clone)]
pub struct UrlFilter {
    registrable_domain: String,
    include: Vec<Regex>,
    exclude: Vec<Regex>,
}

impl UrlFilter {
    /// Scope to the entry URL's registrable domain, with include/exclude
    /// path patterns.
    pub fn new(entry: &Url, include: Vec<Regex>, exclude: Vec<Regex>) -> Self {
        Self {
            registrable_domain: registrable_domain(entry),
            include,
            exclude,
        }
    }

    pub fn scope(&self, url: &Url) -> Scope {
        if !matches!(url.scheme(), "http" | "https") {
            return Scope::WrongScheme;
        }
        if registrable_domain(url) != self.registrable_domain {
            return Scope::OffHost;
        }
        let path = url.path();
        // Exclude wins, and is checked first: a subtree carved out by the
        // config is out even if an include pattern would also match it.
        if self.exclude.iter().any(|p| p.is_match(path)) {
            return Scope::Excluded;
        }
        // No includes means "everything on this host that wasn't excluded".
        if self.include.is_empty() || self.include.iter().any(|p| p.is_match(path)) {
            Scope::InScope
        } else {
            Scope::NotIncluded
        }
    }

    pub fn allows(&self, url: &Url) -> bool {
        self.scope(url).is_in_scope()
    }
}

/// The registrable domain ("python.org" for both `docs.python.org` and
/// `www.python.org`).
///
/// A deliberately simple heuristic — the last two labels — not a full Public
/// Suffix List. It is correct for the vast majority of documentation hosts
/// (`*.readthedocs.io`, `*.python.org`, `doc.rust-lang.org`) and errs on the
/// side of *narrower* scope for multi-label TLDs like `docs.foo.co.uk`
/// (treating `co.uk` as the domain would be too broad; last-two gives
/// `foo.co.uk`'s subtree as `co.uk`, so a `.co.uk` crawl would over-scope).
/// The PSL is the correct fix if a `.co.uk`-hosted doc site ever needs it;
/// until then, this avoids a dependency and its data-file churn. An IP host
/// is its own "domain".
fn registrable_domain(url: &Url) -> String {
    match url.host_str() {
        Some(host) => {
            // An IP literal (or anything with no dot) is its own scope.
            let labels: Vec<&str> = host.split('.').collect();
            if labels.len() <= 2 || host.parse::<std::net::IpAddr>().is_ok() {
                host.to_ascii_lowercase()
            } else {
                labels[labels.len() - 2..].join(".").to_ascii_lowercase()
            }
        }
        None => String::new(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        s.parse().unwrap()
    }

    fn re(s: &str) -> Regex {
        Regex::new(s).unwrap()
    }

    #[test]
    fn same_registrable_domain_is_in_scope_subdomains_included() {
        let f = UrlFilter::new(&url("https://docs.python.org/3/"), vec![], vec![]);
        assert!(f.allows(&url("https://docs.python.org/3/library/os.html")));
        assert!(f.allows(&url("https://www.python.org/downloads/")));
        assert_eq!(
            f.scope(&url("https://github.com/python/cpython")),
            Scope::OffHost
        );
    }

    #[test]
    fn non_http_schemes_are_out_of_scope() {
        let f = UrlFilter::new(&url("https://x.test/"), vec![], vec![]);
        assert_eq!(f.scope(&url("mailto:a@x.test")), Scope::WrongScheme);
        assert_eq!(f.scope(&url("ftp://x.test/f")), Scope::WrongScheme);
    }

    #[test]
    fn exclude_takes_precedence_over_include() {
        let f = UrlFilter::new(
            &url("https://x.test/"),
            vec![re("^/docs/")],
            vec![re("/_sources/")],
        );
        assert!(f.allows(&url("https://x.test/docs/guide.html")));
        assert_eq!(
            f.scope(&url("https://x.test/docs/_sources/guide.txt")),
            Scope::Excluded,
            "exclude wins even though the include also matches"
        );
    }

    #[test]
    fn include_narrows_the_crawl() {
        // The nodejs.org case: crawl /api/, never /docs/.
        let f = UrlFilter::new(&url("https://nodejs.org/"), vec![re("^/api/")], vec![]);
        assert!(f.allows(&url("https://nodejs.org/api/fs.html")));
        assert_eq!(
            f.scope(&url("https://nodejs.org/docs/latest/x.html")),
            Scope::NotIncluded
        );
    }

    #[test]
    fn no_includes_means_everything_on_host() {
        let f = UrlFilter::new(&url("https://x.test/"), vec![], vec![]);
        assert!(f.allows(&url("https://x.test/anything/at/all.html")));
    }

    #[test]
    fn ip_hosts_and_ports_scope_to_themselves() {
        // The loopback fixture server (127.0.0.1:PORT) must be self-scoped,
        // or every crawl test would be off-host from its own entry point.
        let f = UrlFilter::new(&url("http://127.0.0.1:8080/"), vec![], vec![]);
        assert!(f.allows(&url("http://127.0.0.1:8080/page.html")));
        assert_eq!(f.scope(&url("http://10.0.0.1:8080/x")), Scope::OffHost);
    }
}
