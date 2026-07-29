//! The ingestion pipeline, end to end (S1-13).
//!
//! Every stage this composes already existed and is tested on its own; what
//! did not exist was anything that ran them in order and **wrote the result
//! down**. Until this module, `tome-core` could fetch, parse, normalize,
//! sanitize, and localize a documentation site and then drop it on the floor,
//! so the reader had nothing to read and the Stage 1 exit gate — "the app
//! renders docs.python.org with the network off" — was unreachable by
//! construction.
//!
//! ```text
//! SourceConfig ─▶ Crawler ─▶ per page ─▶ normalize ─▶ sanitize ─▶ localize_assets
//!                                                                        │
//!                              PageStore (AST on disk) ◀─────────────────┤
//!                              Database (metadata)     ◀─────────────────┘
//! ```
//!
//! # Where the split falls
//!
//! Content goes to [`PageStore`], metadata to [`Database`]. The reasoning is
//! in `store.rs`: metadata is read in bulk for lists and search, content one
//! page at a time. Both writes happen per page rather than in one batch at
//! the end, so an interrupted pull leaves a partial library that works rather
//! than nothing at all — a 5 000-page site takes a long time at 2 requests a
//! second, and losing all of it to a closed laptop would be indefensible.
//!
//! # Errors are collected, not thrown
//!
//! A page that fails to fetch, or an asset that 404s, must not abort a pull.
//! The crawler already works this way; this module keeps it, and returns
//! everything that went wrong so the caller can print an honest report. A
//! pull that says "done" while having silently skipped 200 pages is worse
//! than one that says what it missed.

use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

use url::Url;

use crate::assets::{localize_assets, AssetStore, FetchingAssetStore};
use crate::config::SourceConfig;
use crate::crawl::Crawler;
use crate::db::Database;
use crate::error::Result;
use crate::fetch::Fetcher;
use crate::model::{DocPage, Node, Page, PagePath, SourceId};
use crate::normalize::normalize;
use crate::sanitize::sanitize;
use crate::search::SearchEngine;
use crate::store::{PageStore, StoredPage};
use crate::Paths;

/// What a pull did, honestly.
#[derive(Debug, Default)]
pub struct IngestReport {
    pub pages_stored: usize,
    /// Pages the crawler could not fetch or parse. Not fatal.
    pub page_errors: Vec<String>,
    /// Assets that could not be localized. The page still renders, with an
    /// "unavailable offline" note where the image was.
    pub asset_errors: Vec<String>,
    /// True when the crawl stopped at `max_pages` rather than running out of
    /// links — so the caller can say "capped" rather than implying "complete".
    pub hit_page_cap: bool,
    /// What indexing did. `None` if it was not reached — a crawl that
    /// produced nothing does not touch the index.
    pub index: Option<IndexReport>,
    pub elapsed: std::time::Duration,
}

/// What incremental indexing did (S2-3).
///
/// The four counts are disjoint and sum to the source's page count plus
/// `removed`, so a caller can print them without them overlapping
/// confusingly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IndexReport {
    /// Pages indexed for the first time.
    pub added: usize,
    /// Pages whose content hash changed, so they were replaced.
    pub updated: usize,
    /// Pages the index held that the source no longer has.
    pub removed: usize,
    /// Pages already indexed at the same content hash — the whole point.
    pub unchanged: usize,
    /// True when the index would not open and was rebuilt from scratch, which
    /// forces every page to count as `added`.
    pub rebuilt: bool,
}

impl IndexReport {
    /// Whether anything was written. A sync that changed nothing still
    /// commits nothing, so there is no new segment and no merge pressure.
    pub fn is_noop(&self) -> bool {
        self.added == 0 && self.updated == 0 && self.removed == 0
    }
}

/// Progress during a pull, for a CLI spinner or a UI progress bar.
#[derive(Debug, Clone)]
pub enum Progress {
    Crawled {
        crawled: usize,
        queued: usize,
        errored: usize,
    },
    Storing {
        stored: usize,
        total: usize,
    },
    Indexing {
        indexed: usize,
        total: usize,
    },
}

/// Fetch a source and write it into the library.
///
/// Everything network-facing is inherited rather than re-implemented: the
/// [`Fetcher`] applies robots.txt, the rate limit, and the SSRF filter, and
/// the [`Crawler`] applies the URL scope. This function adds no policy of its
/// own, which is the point — a second code path to the network would be a
/// second place for those controls to be forgotten.
pub fn pull(
    paths: &Paths,
    config: &SourceConfig,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<IngestReport> {
    let started = Instant::now();
    let mut report = IngestReport::default();

    paths.ensure_source_dirs(&config.id)?;

    let fetcher = Fetcher::new(config.fetch.clone());
    let Some(crawler) = Crawler::new(&fetcher, config) else {
        // Local, docset, and man sources are not crawled; they have their own
        // ingestion paths, which do not exist yet. Saying so beats pretending
        // a pull succeeded with zero pages.
        report.page_errors.push(format!(
            "{} sources cannot be pulled over the network yet",
            config.spec.source_type().as_str()
        ));
        report.elapsed = started.elapsed();
        return Ok(report);
    };

    let outcome = crawler.crawl(&mut |p| {
        on_progress(Progress::Crawled {
            crawled: p.crawled,
            queued: p.queued,
            errored: p.errored,
        });
    });
    report.hit_page_cap = outcome.hit_page_cap;
    for error in &outcome.errors {
        // The URL is the crawler's own, not reading history — it is what the
        // user has to look at to understand why a page is missing.
        report
            .page_errors
            .push(format!("{}: {}", error.url, error.error));
    }

    let database = Database::open(paths)?;
    // Upserted before the pages so that a pull interrupted half way still
    // leaves a source row the sidebar can list, with the pages it did get.
    // `page_count` and `last_synced` are filled in at the end, below.
    let mut source = config.to_source();
    database.upsert_source(&source)?;

    let store = PageStore::new(paths, &config.id);
    let mut asset_store = FetchingAssetStore::new(&fetcher, paths.assets_dir(&config.id));

    let source_url = config.spec.url().cloned();
    // Every page this crawl actually produced. `relink` needs the whole set
    // before it can rewrite any one page's links, since a page in the middle
    // of the crawl links forward to pages fetched after it.
    let held: HashSet<PagePath> = outcome
        .docset
        .pages
        .iter()
        .map(|p| p.meta.path.clone())
        .collect();

    let total = outcome.docset.pages.len();
    for (index, doc_page) in outcome.docset.pages.into_iter().enumerate() {
        let stored = process(
            doc_page,
            source_url.as_ref(),
            &held,
            &mut asset_store,
            &mut report,
        );
        let (page, stored_page) = match stored {
            Some(pair) => pair,
            None => continue,
        };
        store.write(&stored_page)?;
        // The crawl index IS the navigation order: the crawler discovers
        // links from the whole document, and a documentation site advertises
        // its pages through its own contents list. Alphabetical-by-path was
        // opening the Cargo Book on CHANGELOG.html.
        database.upsert_page(&page, u32::try_from(index).unwrap_or(u32::MAX))?;
        report.pages_stored += 1;
        on_progress(Progress::Storing {
            stored: index + 1,
            total,
        });
    }

    // Record what the pull actually produced. Without this, `Source.page_count`
    // stays at its default of 0 for ever and every consumer that trusts it
    // disagrees with every consumer that counts rows — which is exactly what
    // `tome list` and `tome list --json` did before this line existed.
    source.page_count = u32::try_from(report.pages_stored).unwrap_or(u32::MAX);
    source.last_synced = Some(chrono::Utc::now());
    database.upsert_source(&source)?;

    // Index last, and from the database rather than from the crawl's output.
    // Both matter: the pages are on disk and recorded by this point, so an
    // indexing failure costs a search index (rebuildable, in the cache) and
    // never the fetched content (expensive, in the state root). And reading
    // back from the database means a pull that added nothing new still
    // reconciles anything a previous interrupted run left unindexed.
    //
    // The database connection is dropped first: `index_source` opens its own,
    // and SQLite is happier without two write handles on one file.
    drop(database);
    report.index = Some(index_source(
        paths,
        &config.id,
        &config.category,
        on_progress,
    )?);

    report.elapsed = started.elapsed();
    Ok(report)
}

/// Bring the search index up to date with what a source holds on disk (S2-3,
/// spec P2-003).
///
/// # Why this compares hashes rather than just reindexing
///
/// Not to save indexing time. SPIKE-003 finding 1 measured indexing at 5–21
/// seconds for 100 000 pages against roughly seven hours to crawl them, so
/// indexing is effectively free and "avoid re-indexing" would be optimising
/// the wrong end by three orders of magnitude.
///
/// What the comparison buys is that **an unchanged page is not rewritten**,
/// and therefore no segment is created for it. Segment count is what degrades
/// search latency (SPIKE-003 finding 4), and a library that syncs on a
/// schedule forever would otherwise accumulate one segment per sync whether or
/// not anything changed. A no-op sync commits nothing at all.
///
/// # Why the index is the source of truth for "what is indexed"
///
/// See [`SearchEngine::indexed_pages`]. The database and the index live under
/// different roots — state and cache — and can legitimately diverge.
///
/// # Commit strategy
///
/// One commit, at the end. P2-003 asks for a batch size or a timer; the right
/// batch here is the whole sync, because a commit is what creates a segment.
/// Committing every N pages would turn one sync into N segments and pay for it
/// on every subsequent search.
pub fn index_source(
    paths: &Paths,
    source: &SourceId,
    category: &str,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<IndexReport> {
    let (engine, rebuilt) = SearchEngine::open_or_rebuild(paths)?;
    let mut report = IndexReport {
        rebuilt,
        ..IndexReport::default()
    };

    let database = Database::open(paths)?;
    let pages = database.list_pages(source)?;
    let store = PageStore::new(paths, source);

    // After a rebuild the index holds nothing, so every page is new. Asking
    // it anyway would be correct but pointless.
    let indexed = if rebuilt {
        BTreeMap::new()
    } else {
        engine.indexed_pages(source)?
    };

    // Every path the source currently holds, for the deletion pass below.
    let live: HashSet<&str> = pages.iter().map(|page| page.path.as_str()).collect();

    // `true` means "already indexed under a different hash", so the old
    // document must be deleted before the new one is added.
    let work: Vec<(&Page, bool)> = pages
        .iter()
        .filter_map(|page| match indexed.get(page.path.as_str()) {
            Some(hash) if hash == page.content_hash.as_str() => {
                report.unchanged += 1;
                None
            }
            Some(_) => Some((page, true)),
            None => Some((page, false)),
        })
        .collect();

    // Pages the index holds that the source no longer lists. Derived from the
    // database's page set rather than from this crawl's output, so a page
    // dropped by an earlier interrupted pull is still cleaned up.
    let removed: Vec<PagePath> = indexed
        .keys()
        .filter(|path| !live.contains(path.as_str()))
        .filter_map(|path| PagePath::new(path.as_str()).ok())
        .collect();

    if work.is_empty() && removed.is_empty() {
        return Ok(report);
    }

    let total = work.len();
    let mut session = engine.session()?;

    for path in &removed {
        session.delete_page(source, path)?;
        report.removed += 1;
    }

    for (done, (page, changed)) in work.iter().enumerate() {
        let (page, changed) = (*page, *changed);
        // A changed page must be deleted before it is added: Tantivy has no
        // update, so writing the new document without removing the old leaves
        // BOTH in the index and the same page appears twice in every result
        // list. P2-003 calls this out as "no duplicate documents in index".
        if changed {
            session.delete_page(source, &page.path)?;
            report.updated += 1;
        } else {
            report.added += 1;
        }

        // A page the store cannot produce is skipped rather than fatal: the
        // index is derived, and one unreadable page should not stop the rest
        // from becoming searchable.
        let Some(stored) = store.read(&page.path)? else {
            continue;
        };
        session.add_page(page, category, &stored.body)?;

        on_progress(Progress::Indexing {
            indexed: done + 1,
            total,
        });
    }

    session.commit()?;
    Ok(report)
}

/// One page through normalize → sanitize → localize → relink.
///
/// The order is not interchangeable. Normalization resolves URLs to absolute
/// so that sanitization sees the scheme it is judging and localization has
/// something to fetch; sanitization runs *before* localization so a
/// `javascript:` image src is gone before anything tries to retrieve it;
/// relinking runs last, on hrefs the sanitizer has already vetted.
fn process(
    doc_page: DocPage,
    source_url: Option<&Url>,
    held: &HashSet<PagePath>,
    assets: &mut dyn AssetStore,
    report: &mut IngestReport,
) -> Option<(Page, StoredPage)> {
    let DocPage { mut meta, body } = doc_page;

    let base = page_base(source_url, &meta);
    let normalized = normalize(body, &base);
    let sanitized = sanitize(normalized.body);
    let mut localized = localize_assets(sanitized, &base, assets);
    localized.body = relink(localized.body, &base, held);

    for error in localized.errors {
        report
            .asset_errors
            .push(format!("{}: {}", error.url, error.reason));
    }

    // Normalization extracts a better title than the crawler's guess (the
    // page's own h1, rather than whatever the `<title>` element said after a
    // site appended " — Project 3.12 documentation" to it).
    if let Some(title) = normalized.title.filter(|t| !t.trim().is_empty()) {
        meta.title = title;
    }

    let stored = StoredPage {
        path: meta.path.clone(),
        title: meta.title.clone(),
        description: normalized.description,
        body: localized.body,
    };
    Some((meta, stored))
}

/// Rewrite links that point at pages this library holds into **library
/// paths**, and leave every other link alone.
///
/// Normalization makes every href absolute, which is right for fetching and
/// wrong for storing: `http://docs.example.org/api/ref.html` names a host,
/// and a stored page that names a host is a page that only makes sense on the
/// machine that crawled it. Bookmark sync (ADR-0001) copies these trees
/// between devices, and the reader has to route a click to a *page*, not to a
/// URL. So an internal link becomes `api/ref.html#section` — exactly the
/// `(source, path)` identity the rest of the system is keyed by.
///
/// The test is deliberately "does the library actually hold this page", not
/// "is it the same host". A same-host link the crawl never reached — excluded
/// by the URL filter, or past the depth limit — stays absolute and is treated
/// as external, because that is the truth: Tome does not have it, and opening
/// it in a browser is more useful than a dead in-app link.
fn relink(node: Node, base: &Url, held: &HashSet<PagePath>) -> Node {
    map_links(node, &mut |href| {
        // A bare `#fragment` is same-page navigation and must stay untouched;
        // joining it against the base would turn it into an absolute URL and
        // break every permalink on the page.
        if href.starts_with('#') {
            return href;
        }
        let Ok(absolute) = base.join(&href) else {
            return href;
        };
        if !matches!(absolute.scheme(), "http" | "https") {
            return href;
        }
        let path = crate::crawl::page_path_for(&absolute);
        if !held.contains(&path) {
            return href;
        }
        match absolute.fragment() {
            Some(fragment) => format!("{path}#{fragment}"),
            None => path.to_string(),
        }
    })
}

/// Apply `f` to every [`Node::Link`] href in the tree.
fn map_links(node: Node, f: &mut dyn FnMut(String) -> String) -> Node {
    use crate::model::{Definition, ListItem, TableCell, TableRow};

    let children = |children: Vec<Node>, f: &mut dyn FnMut(String) -> String| {
        children.into_iter().map(|c| map_links(c, f)).collect()
    };

    match node {
        Node::Link {
            href,
            title,
            children: kids,
        } => Node::Link {
            href: f(href),
            title,
            children: children(kids, f),
        },
        Node::Document { children: kids } => Node::Document {
            children: children(kids, f),
        },
        Node::Heading {
            level,
            id,
            children: kids,
        } => Node::Heading {
            level,
            id,
            children: children(kids, f),
        },
        Node::Paragraph { children: kids } => Node::Paragraph {
            children: children(kids, f),
        },
        Node::Blockquote { children: kids } => Node::Blockquote {
            children: children(kids, f),
        },
        Node::Emphasis { children: kids } => Node::Emphasis {
            children: children(kids, f),
        },
        Node::Strong { children: kids } => Node::Strong {
            children: children(kids, f),
        },
        Node::Admonition {
            kind,
            title,
            children: kids,
        } => Node::Admonition {
            kind,
            title,
            children: children(kids, f),
        },
        Node::List {
            ordered,
            start,
            items,
        } => Node::List {
            ordered,
            start,
            items: items
                .into_iter()
                .map(|item| ListItem {
                    children: children(item.children, f),
                })
                .collect(),
        },
        Node::DefinitionList { items } => Node::DefinitionList {
            items: items
                .into_iter()
                .map(|d| Definition {
                    id: d.id,
                    term: children(d.term, f),
                    definition: children(d.definition, f),
                })
                .collect(),
        },
        Node::Table { headers, rows } => Node::Table {
            headers: headers
                .into_iter()
                .map(|c| TableCell {
                    children: children(c.children, f),
                })
                .collect(),
            rows: rows
                .into_iter()
                .map(|r| TableRow {
                    cells: r
                        .cells
                        .into_iter()
                        .map(|c| TableCell {
                            children: children(c.children, f),
                        })
                        .collect(),
                })
                .collect(),
        },
        other => other,
    }
}

/// The URL a page's relative references resolve against: the page's own
/// original address.
///
/// [`Page`] deliberately does not carry it — identity is `(source, path)`,
/// per `model/mod.rs` — so it is reconstructed here, and the reconstruction
/// is the **exact inverse of [`crate::crawl::Crawler::page_path`]**: that
/// function takes a URL's path, drops the leading `/`, and appends
/// `index.html` to a directory URL. So the source's origin with the path put
/// back gives the page's address again. (The appended `index.html` resolves
/// relative references to the same directory the bare URL would, so it makes
/// no difference to the only thing a base is used for.)
///
/// **The first version of this synthesised a fake base** (`tome-page:/…`)
/// on the theory that normalization had already made every URL absolute.
/// It had — but relative ones were then made absolute against the *fake*
/// base, so `_static/logo.svg` became `tome-page:/_static/logo.svg`, the
/// sanitizer rejected the scheme, and every image on every page silently
/// degraded to its alt text. `reader_offline.rs` caught it; that is what the
/// end-to-end test is for.
fn page_base(source_url: Option<&Url>, page: &Page) -> Url {
    // The workspace denies `expect` in library code and asks for a per-call
    // allow with a justification. This is one: the argument is a literal with
    // no fallible input, used only when a source has no URL at all.
    #[allow(clippy::expect_used)]
    let fallback = || Url::parse("tome-page:/").expect("a URL literal with no fallible input");

    let Some(source_url) = source_url else {
        return fallback();
    };
    let mut base = source_url.clone();
    base.set_query(None);
    base.set_fragment(None);
    base.set_path(&format!("/{}", page.path));
    base
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::model::{ContentHash, SourceId};

    fn page_meta(path: &str) -> Page {
        Page::new(
            SourceId::new("demo").unwrap(),
            PagePath::new(path).unwrap(),
            "crawler guess",
            ContentHash::from_digest([0; 32]),
        )
    }

    /// An asset store that localizes nothing, so `process` can be tested
    /// without a network.
    struct RefusingAssets;
    impl AssetStore for RefusingAssets {
        fn localize(&mut self, url: &Url) -> std::result::Result<String, String> {
            Err(format!("refused {url}"))
        }
    }

    #[test]
    fn a_pages_own_h1_beats_the_crawlers_title_guess() {
        // Sphinx appends " — Project 3.12 documentation" to every <title>.
        // The h1 is what belongs in the sidebar.
        let doc = DocPage {
            meta: page_meta("tutorial.html"),
            body: Node::Document {
                children: vec![Node::Heading {
                    level: 1,
                    id: None,
                    children: vec![Node::Text {
                        value: "Real Title".into(),
                    }],
                }],
            },
        };
        let mut report = IngestReport::default();
        let (meta, stored) =
            process(doc, None, &HashSet::new(), &mut RefusingAssets, &mut report).unwrap();
        assert_eq!(meta.title, "Real Title");
        assert_eq!(stored.title, "Real Title");
    }

    #[test]
    fn a_failed_asset_is_reported_and_never_left_remote() {
        // The offline guarantee: an asset that cannot be localized becomes a
        // placeholder, and the reason is surfaced rather than swallowed.
        let doc = DocPage {
            meta: page_meta("page.html"),
            body: Node::Document {
                children: vec![Node::Paragraph {
                    children: vec![Node::Image {
                        src: "https://cdn.example/logo.png".into(),
                        alt: "logo".into(),
                    }],
                }],
            },
        };
        let mut report = IngestReport::default();
        let (_, stored) =
            process(doc, None, &HashSet::new(), &mut RefusingAssets, &mut report).unwrap();

        assert_eq!(report.asset_errors.len(), 1);
        assert!(report.asset_errors[0].contains("cdn.example"));
        let json = serde_json::to_string(&stored.body).unwrap();
        assert!(!json.contains("cdn.example"), "{json}");
        assert!(json.contains("unavailable offline"), "{json}");
    }

    #[test]
    fn sanitization_runs_before_anything_tries_to_fetch() {
        // A `javascript:` image src must be gone before the asset store sees
        // it -- so the store is one that panics if asked to fetch anything.
        struct NeverCalled;
        impl AssetStore for NeverCalled {
            fn localize(&mut self, url: &Url) -> std::result::Result<String, String> {
                unreachable!("the sanitizer should have removed {url}");
            }
        }
        let doc = DocPage {
            meta: page_meta("page.html"),
            body: Node::Document {
                children: vec![Node::Paragraph {
                    children: vec![Node::Image {
                        src: "javascript:alert(1)".into(),
                        alt: "x".into(),
                    }],
                }],
            },
        };
        let mut report = IngestReport::default();
        let (_, stored) =
            process(doc, None, &HashSet::new(), &mut NeverCalled, &mut report).unwrap();
        let json = serde_json::to_string(&stored.body).unwrap();
        assert!(!json.contains("javascript"), "{json}");
    }

    #[test]
    fn the_page_base_inverts_the_crawlers_page_path() {
        // The coupling this rests on. If `Crawler::page_path` ever changes
        // how it turns a URL into a path, this test fails rather than every
        // image on every page silently degrading to alt text -- which is
        // exactly what happened when the base was synthesised instead.
        let source: Url = "https://docs.example.org/en/latest/?v=1#frag"
            .parse()
            .unwrap();
        for (path, expected) in [
            ("index.html", "https://docs.example.org/index.html"),
            ("api/ref.html", "https://docs.example.org/api/ref.html"),
            (
                "guide/index.html",
                "https://docs.example.org/guide/index.html",
            ),
        ] {
            let base = page_base(Some(&source), &page_meta(path));
            assert_eq!(base.as_str(), expected);
            // The only job a base has: resolving a sibling reference.
            assert_eq!(
                base.join("_static/logo.svg").unwrap().scheme(),
                "https",
                "a relative asset must resolve to something fetchable"
            );
        }
    }

    fn link(href: &str) -> Node {
        Node::Document {
            children: vec![Node::Paragraph {
                children: vec![Node::Link {
                    href: href.to_owned(),
                    title: None,
                    children: vec![Node::Text { value: "x".into() }],
                }],
            }],
        }
    }

    fn href_of(node: &Node) -> String {
        match node {
            Node::Link { href, .. } => href.clone(),
            Node::Document { children } | Node::Paragraph { children } => href_of(&children[0]),
            other => panic!("no link in {other:?}"),
        }
    }

    fn relinked(href: &str, held: &[&str]) -> String {
        let base: Url = "https://docs.example.org/api/ref.html".parse().unwrap();
        let held: HashSet<PagePath> = held.iter().map(|p| PagePath::new(*p).unwrap()).collect();
        href_of(&relink(link(href), &base, &held))
    }

    #[test]
    fn a_link_to_a_page_we_hold_becomes_a_library_path() {
        // The stored tree must not name a host: bookmark sync copies it
        // between devices, and the reader routes to a page, not a URL.
        assert_eq!(
            relinked(
                "https://docs.example.org/guide/intro.html",
                &["guide/intro.html"]
            ),
            "guide/intro.html"
        );
        assert_eq!(
            relinked("../guide/intro.html", &["guide/intro.html"]),
            "guide/intro.html"
        );
        assert_eq!(
            relinked("https://docs.example.org/guide/", &["guide/index.html"]),
            "guide/index.html"
        );
    }

    #[test]
    fn a_fragment_survives_the_rewrite() {
        assert_eq!(
            relinked(
                "https://docs.example.org/guide/intro.html#setup",
                &["guide/intro.html"]
            ),
            "guide/intro.html#setup"
        );
    }

    #[test]
    fn a_same_page_fragment_is_left_exactly_alone() {
        // Joining `#x` against the base would make it absolute and break
        // every permalink on the page -- and Sphinx puts one on every
        // heading and every API entry.
        assert_eq!(
            relinked("#widget.Widget", &["api/ref.html"]),
            "#widget.Widget"
        );
    }

    #[test]
    fn a_same_host_page_we_do_not_hold_stays_external() {
        // The test is "does the library have it", not "is it the same host".
        // A link the crawl never reached is one Tome cannot open; leaving it
        // absolute means it opens in a browser, which is the truth.
        assert_eq!(
            relinked("https://docs.example.org/blog/post.html", &["api/ref.html"]),
            "https://docs.example.org/blog/post.html"
        );
    }

    #[test]
    fn an_external_link_is_untouched() {
        assert_eq!(
            relinked("https://example.invalid/other", &["api/ref.html"]),
            "https://example.invalid/other"
        );
        assert_eq!(
            relinked("mailto:someone@example.org", &["api/ref.html"]),
            "mailto:someone@example.org"
        );
    }

    #[test]
    fn relinking_reaches_links_nested_anywhere() {
        // A link inside a table cell inside a definition is still a link.
        let base: Url = "https://docs.example.org/api/ref.html".parse().unwrap();
        let held: HashSet<PagePath> = [PagePath::new("guide/intro.html").unwrap()]
            .into_iter()
            .collect();
        let deep = Node::Document {
            children: vec![Node::DefinitionList {
                items: vec![crate::model::Definition {
                    id: None,
                    term: vec![Node::Link {
                        href: "https://docs.example.org/guide/intro.html".into(),
                        title: None,
                        children: vec![],
                    }],
                    definition: vec![Node::Table {
                        headers: vec![],
                        rows: vec![crate::model::TableRow {
                            cells: vec![crate::model::TableCell {
                                children: vec![Node::Link {
                                    href: "https://docs.example.org/guide/intro.html".into(),
                                    title: None,
                                    children: vec![],
                                }],
                            }],
                        }],
                    }],
                }],
            }],
        };
        let json = serde_json::to_string(&relink(deep, &base, &held)).unwrap();
        assert!(!json.contains("docs.example.org"), "{json}");
        assert_eq!(json.matches("guide/intro.html").count(), 2, "{json}");
    }

    #[test]
    fn a_source_with_no_url_still_yields_a_usable_base() {
        // Local and man sources have no URL. The base is unused for them --
        // there is nothing relative to resolve -- but it must not be a panic.
        let base = page_base(None, &page_meta("a b.html"));
        assert_eq!(base.scheme(), "tome-page");
    }
}
