//! Breadth-first crawl (implementation plan S1-6, tickets P1-010/P1-011).
//!
//! Drives [`Fetcher`](crate::fetch::Fetcher) and [`parse_page`] over a
//! documentation site: start at the entry points, fetch each page, extract
//! its in-scope links, and queue the ones not yet seen, up to a depth and a
//! page cap. The output is a [`DocSet`] plus a per-page error list — a crawl
//! of a real site always hits a few 404s and timeouts, and losing the whole
//! run because one page failed would be the wrong trade.
//!
//! What this module is NOT: it does not normalize (S1-8), sanitize (S1-9),
//! localize assets (S1-10), or persist (S1-2 owns the repos). It produces the
//! raw parsed AST per page; the pipeline stages downstream refine it. Keeping
//! the crawl a pure "fetch + discover" step is what lets the fixture server
//! test it end to end without a database.
//!
//! Politeness and safety are inherited, not re-implemented: the `Fetcher`
//! already enforces robots.txt, per-host rate limiting, and the SSRF filter
//! (S1-5), so the crawler never has to think about them — it just calls
//! `fetch`. The one scoping decision that IS the crawler's is which links to
//! follow, and that is [`UrlFilter`].

pub mod url_filter;

use std::collections::{HashSet, VecDeque};

use url::Url;

use crate::config::SourceConfig;
use crate::error::Error;
use crate::fetch::{FetchOutcome, Fetcher};
use crate::model::{DocPage, DocSet, Page, PagePath, TocEntry};
use crate::parse::parse_page_with;
use url_filter::UrlFilter;

pub use url_filter::Scope;

/// A page that could not be crawled, kept rather than aborting the run.
#[derive(Debug)]
pub struct CrawlError {
    pub url: Url,
    pub error: Error,
}

/// Live progress, delivered to the callback after each page is processed.
#[derive(Debug, Clone, Copy)]
pub struct CrawlProgress {
    /// Pages successfully fetched and parsed so far.
    pub crawled: usize,
    /// URLs discovered and queued but not yet fetched.
    pub queued: usize,
    /// Pages that errored so far.
    pub errored: usize,
}

/// The result of a crawl: what was fetched, what failed, and the totals.
#[derive(Debug)]
pub struct CrawlOutcome {
    pub docset: DocSet,
    pub errors: Vec<CrawlError>,
    /// True if the crawl stopped because it hit `max_pages`, so the caller
    /// can tell "site is this size" from "we capped it". Never a silent
    /// truncation.
    pub hit_page_cap: bool,
}

/// A breadth-first crawler for one source.
pub struct Crawler<'a> {
    fetcher: &'a Fetcher,
    config: &'a SourceConfig,
    max_depth: u32,
    max_pages: u32,
    max_asset_bytes: u64,
    content_selector: Option<String>,
    /// Platform furniture rules for this source's type (S2-11). `None` is the
    /// generic path.
    profile: Option<&'static crate::scrape::Profile>,
}

impl<'a> Crawler<'a> {
    /// Build a crawler for a generic/readthedocs/etc. source. Returns `None`
    /// for source types that are not crawled (local, docset, man) — those
    /// have their own ingestion paths.
    pub fn new(fetcher: &'a Fetcher, config: &'a SourceConfig) -> Option<Self> {
        use crate::config::SourceSpec;
        let (max_depth, max_pages, content_selector) = match &config.spec {
            SourceSpec::Generic { scraper, .. } => (
                scraper.max_depth,
                scraper.max_pages,
                scraper.content_selector.clone(),
            ),
            // ReadTheDocs / rustdoc / mdBook crawl with the generic limits and
            // no content selector: their content roots are already what the
            // generic root selection finds (`role="main"` for Sphinx, `<main>`
            // for the other two). What they *do* get is a furniture profile,
            // below — see `crate::scrape` for why that is the whole difference.
            SourceSpec::ReadTheDocs { .. }
            | SourceSpec::Rustdoc { .. }
            | SourceSpec::MdBook { .. } => (4, 5000, None),
            SourceSpec::Man(_) | SourceSpec::Local { .. } | SourceSpec::Docset { .. } => {
                return None
            }
        };
        Some(Self {
            fetcher,
            config,
            max_depth,
            max_pages,
            max_asset_bytes: config.fetch.max_asset_bytes,
            content_selector,
            profile: crate::scrape::profile_for(config.spec.source_type()),
        })
    }

    /// Run the crawl, reporting progress to `on_progress` after each page.
    pub fn crawl(&self, on_progress: &mut dyn FnMut(CrawlProgress)) -> CrawlOutcome {
        let entry = match self.entry_urls() {
            Ok(urls) => urls,
            Err(error) => {
                return CrawlOutcome {
                    docset: DocSet::new(Vec::new(), Vec::new()),
                    errors: vec![CrawlError {
                        // A source with no crawlable URL is a config-shaped
                        // problem surfaced as a crawl error, with a stand-in
                        // URL so the report is uniform.
                        url: "about:blank".parse().unwrap_or_else(|_| placeholder_url()),
                        error,
                    }],
                    hit_page_cap: false,
                };
            }
        };

        let filter = UrlFilter::new(
            &entry[0],
            self.config
                .spec
                .generic_include()
                .cloned()
                .unwrap_or_default(),
            self.config
                .spec
                .generic_exclude()
                .cloned()
                .unwrap_or_default(),
        );

        // BFS: (url, depth). Entry points are depth 1, so `max_depth: 1`
        // (the config floor) means "the entry pages only" and `max_depth: 4`
        // (the default) follows links four levels deep. visited dedups by the
        // normalized URL string so a page linked from ten others is fetched
        // once.
        let mut queue: VecDeque<(Url, u32)> = VecDeque::new();
        let mut visited: HashSet<String> = HashSet::new();
        for url in &entry {
            if visited.insert(url.as_str().to_owned()) {
                queue.push_back((url.clone(), 1));
            }
        }

        let mut pages: Vec<DocPage> = Vec::new();
        let mut toc: Vec<TocEntry> = Vec::new();
        let mut errors: Vec<CrawlError> = Vec::new();
        let mut hit_page_cap = false;

        while let Some((url, depth)) = queue.pop_front() {
            if pages.len() as u32 >= self.max_pages {
                hit_page_cap = true;
                break;
            }

            match self.fetch_and_parse(&url) {
                Ok(Some(page)) => {
                    // Queue in-scope, unseen links — but only if this page is
                    // not already at the depth limit (its links would be
                    // depth+1, past the limit).
                    if depth < self.max_depth {
                        for link in &page.links {
                            if filter.allows(link) && visited.insert(link.as_str().to_owned()) {
                                queue.push_back((link.clone(), depth + 1));
                            }
                        }
                    }
                    toc.push(TocEntry::new(page.title.clone(), Some(page.path.clone())));
                    pages.push(page.doc_page);
                }
                // 304 Not Modified with no validators shouldn't happen on a
                // fresh crawl, but if it does the page carries no new content;
                // skip it without erroring.
                Ok(None) => {}
                Err(error) => errors.push(CrawlError { url, error }),
            }

            on_progress(CrawlProgress {
                crawled: pages.len(),
                queued: queue.len(),
                errored: errors.len(),
            });
        }

        CrawlOutcome {
            docset: DocSet::new(pages, toc),
            errors,
            hit_page_cap,
        }
    }

    fn entry_urls(&self) -> Result<Vec<Url>, Error> {
        let base = self.config.spec.url().ok_or_else(|| Error::Config {
            file: std::path::PathBuf::from(format!("{}.yaml", self.config.id)),
            message: "this source type has no crawlable URL".into(),
        })?;
        let entry_points = self.config.spec.generic_entry_points();
        if entry_points.is_empty() {
            return Ok(vec![base.clone()]);
        }
        // Entry points are paths relative to the base URL.
        let mut urls = Vec::new();
        for point in entry_points {
            match base.join(point) {
                Ok(url) => urls.push(url),
                Err(_) => {
                    // A malformed entry point is a config error, but one bad
                    // entry among several should not sink the crawl.
                    tracing::warn!(point, "skipping unparseable entry point");
                }
            }
        }
        if urls.is_empty() {
            Ok(vec![base.clone()])
        } else {
            Ok(urls)
        }
    }

    fn fetch_and_parse(&self, url: &Url) -> Result<Option<CrawledPage>, Error> {
        let outcome = self.fetcher.fetch(url, self.max_asset_bytes, None)?;
        let fetched = match outcome {
            FetchOutcome::Fetched(f) => f,
            FetchOutcome::NotModified => return Ok(None),
        };

        // Only parse HTML; a crawl that follows a link to a PDF or a tarball
        // should record it as fetched-but-not-a-page, not feed binary to the
        // HTML parser.
        if let Some(ct) = &fetched.content_type {
            if !ct.to_ascii_lowercase().contains("html") {
                tracing::debug!(%url, content_type = ct, "skipping non-HTML resource");
                return Ok(None);
            }
        }

        let html = String::from_utf8_lossy(&fetched.body);
        // The platform profile comes from the source's declared type, so a
        // rustdoc source gets rustdoc's furniture rules without the user
        // writing a selector. `None` is the generic path, unchanged.
        let parsed = parse_page_with(
            &html,
            &fetched.final_url,
            self.content_selector.as_deref(),
            self.profile,
        );

        let path = self.page_path(&fetched.final_url);
        let title = parsed.title.clone().unwrap_or_else(|| path.to_string());
        let content_hash =
            crate::model::ContentHash::from_digest(crate::hash::sha256(&fetched.body));

        let mut meta = Page::new(
            self.config.id.clone(),
            path.clone(),
            title.clone(),
            content_hash,
        );
        meta.etag = fetched.etag.clone();
        meta.last_modified = fetched.last_modified.clone();

        Ok(Some(CrawledPage {
            path,
            title,
            links: parsed.links,
            doc_page: DocPage {
                meta,
                body: parsed.body,
            },
        }))
    }

    fn page_path(&self, url: &Url) -> PagePath {
        page_path_for(url)
    }
}

/// Derive a source-relative [`PagePath`] from a fetched URL. The path is the
/// URL path with its leading slash removed; a directory URL (`/guide/`)
/// becomes `guide/index.html` so every page has a file-shaped path the store
/// can write.
///
/// **Free function, not just a method, because two other places need exactly
/// this mapping and a second implementation of it would drift:** the ingestion
/// pipeline inverts it to rebuild a page's original URL (for resolving
/// relative assets), and applies it again to decide whether an internal link
/// points at a page the library actually holds. See `pipeline::page_base`.
pub fn page_path_for(url: &Url) -> PagePath {
    let mut path = url.path().trim_start_matches('/').to_owned();
    if path.is_empty() || path.ends_with('/') {
        path.push_str("index.html");
    }
    // PagePath rejects dot segments and the like; a URL that produces one
    // (rare — the fetcher resolved it already) falls back to a hash-named
    // page rather than dropping the content.
    PagePath::new(&path).unwrap_or_else(|_| {
        let digest = crate::hash::sha256(url.as_str().as_bytes());
        let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
        #[allow(clippy::unwrap_used)] // the constructed name is always valid
        PagePath::new(format!("pages/{hex}.html")).unwrap()
    })
}

/// One crawled page before it becomes a `DocPage` — carries the discovered
/// links (needed for BFS) beside the page itself.
struct CrawledPage {
    path: PagePath,
    title: String,
    links: Vec<Url>,
    doc_page: DocPage,
}

fn placeholder_url() -> Url {
    #[allow(clippy::unwrap_used)] // a compile-time-constant valid URL
    "http://invalid.invalid/".parse().unwrap()
}
