//! Documentation platform detection (S2-10, spec P2-014).
//!
//! # Every marker here was measured, not guessed
//!
//! The rules below were chosen by counting occurrences across S2-9's 128
//! committed homepages, not by reading each generator's documentation. That
//! matters because the obvious markers are wrong in both directions:
//!
//! | Candidate | Where it actually appears |
//! |---|---|
//! | `contains("rustdoc")` | 20/20 rustdoc pages — **and the mdBook-built rustdoc book**, where it is prose |
//! | `contains("sphinx")` | 38/40 Sphinx pages, and 0 elsewhere — but 2 Sphinx sites have no marker at all |
//! | `contains("readthedocs")` | 36/40 Sphinx — **and a MkDocs site hosted on readthedocs.io** |
//! | `md-component` | 10/12 MkDocs — but not MkDocs' *own* site, which uses the default theme |
//!
//! P2-014's sketch does `url.contains("readthedocs") → ReadTheDocs, 0.95` as
//! its first rule. The corpus contains a counter-example to exactly that.
//!
//! # Detection is allowed to say "I don't know"
//!
//! P2-020 records the original detector as returning `(Generic, 1.0)` — full
//! confidence in the fallback — which makes "I have no idea" indistinguishable
//! from "I am certain". A confidence below [`AUTO_ACCEPT`] means the UI asks
//! rather than assumes, and the fallback sits well under it.
//!
//! The metric that matters most is **no confident-but-wrong classification**:
//! guessing Sphinx for a company homepage and crawling it as documentation is
//! worse than admitting uncertainty. `tests/detection.rs` gates that with no
//! margin at all.
//!
//! # What it costs
//!
//! One request — the homepage — and a handful of substring scans over it. No
//! crawl, no probing for `book.toml` or `searchindex.js`, which P2-014 lists
//! as evidence but which cost a round trip each and, measurably, add nothing:
//! every platform in the corpus is separable from the homepage alone except
//! the two Sphinx sites that carry no marker anywhere.

use std::collections::BTreeMap;

/// A documentation generator, as far as the scrapers are concerned.
///
/// ReadTheDocs is deliberately **not** a variant. It is Sphinx, hosted — the
/// markup a scraper reads is Sphinx's, and P2-010 handles both with one
/// scraper. A separate label would put two names on one behaviour and make the
/// confusion matrix report disagreements that do not matter.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    Sphinx,
    Rustdoc,
    MdBook,
    GitBook,
    Docusaurus,
    MkDocs,
    /// Not one of the above. Includes documentation built by something else
    /// entirely, and pages that are not documentation at all.
    Generic,
}

impl Platform {
    /// Every variant, for iterating a confusion matrix.
    pub const ALL: [Self; 7] = [
        Self::Sphinx,
        Self::Rustdoc,
        Self::MdBook,
        Self::GitBook,
        Self::Docusaurus,
        Self::MkDocs,
        Self::Generic,
    ];

    /// The stable name used in fixtures and reports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sphinx => "sphinx",
            Self::Rustdoc => "rustdoc",
            Self::MdBook => "mdbook",
            Self::GitBook => "gitbook",
            Self::Docusaurus => "docusaurus",
            Self::MkDocs => "mkdocs",
            Self::Generic => "generic",
        }
    }

    /// Parse the fixture form. `None` rather than a fallback to `Generic`: a
    /// misspelt label in a fixture would otherwise be silently scored as a
    /// non-documentation site and quietly inflate the corpus's Generic count.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == text)
    }
}

/// What the detector concluded, and how sure it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detection {
    pub platform: Platform,
    /// 0.0–1.0. Compare against [`AUTO_ACCEPT`] rather than to another
    /// detection: these are calibrated confidences, not probabilities, and
    /// the only decision they drive is "act, or ask".
    pub confidence: f32,
}

impl Detection {
    /// Whether this is sure enough to act on without asking the user.
    pub fn is_confident(&self) -> bool {
        self.confidence >= AUTO_ACCEPT
    }
}

/// The confidence at or above which detection is acted on without asking.
///
/// Deliberately high. The cost of asking is one dialog; the cost of being
/// confidently wrong is crawling a site with the wrong scraper and filling a
/// user's library with mis-parsed pages that look fine until they are read.
pub const AUTO_ACCEPT: f32 = 0.8;

/// What a detector gets to look at.
///
/// The homepage and its response headers, and nothing else — no crawl. That
/// bound is what makes detection cheap enough to run before asking the user to
/// commit to a source, and it is what the corpus fixtures record.
#[derive(Debug, Clone)]
pub struct Evidence<'a> {
    pub url: &'a str,
    /// Lowercased header names.
    pub headers: &'a BTreeMap<String, String>,
    pub html: &'a str,
}

/// One rule: a marker, what it means, and how much it is worth.
struct Rule {
    platform: Platform,
    /// Lowercase substrings, any one of which fires the rule.
    markers: &'static [&'static str],
    confidence: f32,
}

/// The rules, in priority order. **The order is load-bearing** — see
/// [`RUSTDOC_BOOK`] for the case that forces it.
///
/// Confidences are calibrated against the corpus: a marker that appeared in
/// every fixture of its class and nowhere else is worth more than one that
/// appeared in ten of twelve.
const RULES: &[Rule] = &[
    // rustdoc's asset paths are unambiguous and machine-generated. `rustdoc-`
    // and `normalize-` are hashed asset filenames that appeared in 20/20
    // rustdoc fixtures and no others.
    Rule {
        platform: Platform::Rustdoc,
        markers: &["/rustdoc-", "normalize-", "rustdoc-vars", "data-rustdoc"],
        confidence: 0.97,
    },
    // mdBook's own furniture. 20/20, nothing else.
    Rule {
        platform: Platform::MdBook,
        markers: &["sidebar-scrollbox", "mdbook.js", "book.js", "mdbook-"],
        confidence: 0.95,
    },
    // Sphinx writes this file on every build and no other generator does.
    // 38/40, zero false positives across the other 88 fixtures.
    Rule {
        platform: Platform::Sphinx,
        markers: &[
            "documentation_options.js",
            "searchtools.js",
            "sphinxsidebar",
            "sphinx-highlight.js",
            "_static/pygments.css",
        ],
        confidence: 0.95,
    },
    Rule {
        platform: Platform::Docusaurus,
        markers: &["__docusaurus", "docusaurus_skiptocontent", "docusaurus-mt-"],
        confidence: 0.95,
    },
    // Material for MkDocs, and Zensical, which emits the same markup.
    Rule {
        platform: Platform::MkDocs,
        markers: &["md-component", "md-header", "md-nav__", "md-typeset"],
        confidence: 0.9,
    },
    Rule {
        platform: Platform::GitBook,
        markers: &[
            "gitbook-root",
            "__gitbook",
            "gitbook.com/",
            "gitbook-plugin",
        ],
        confidence: 0.9,
    },
    // Weaker, later: MkDocs' default theme has no Material classes, and says
    // so only in a footer credit. Worth less because "mkdocs" also appears in
    // prose on pages *about* MkDocs.
    Rule {
        platform: Platform::MkDocs,
        markers: &[
            "built with <a href=\"https://www.mkdocs.org",
            "mkdocs-",
            "/mkdocs/",
        ],
        confidence: 0.85,
    },
    // Sphinx with a theme that hides everything else. `_static/` is Sphinx's
    // asset directory and survives most themes.
    Rule {
        platform: Platform::Sphinx,
        markers: &["_static/", "sphinx"],
        confidence: 0.85,
    },
];

/// `doc.rust-lang.org/rustdoc/` is the **rustdoc book**: documentation *about*
/// rustdoc, built with mdBook.
///
/// It contains the word "rustdoc" everywhere, which is why the rustdoc rule
/// matches on hashed asset paths (`/rustdoc-`, `normalize-`) rather than on
/// the name — and why mdBook's furniture is checked before anything that could
/// match prose. The corpus has this page precisely so the mistake cannot be
/// made silently.
const RUSTDOC_BOOK: &str = "sidebar-scrollbox";

/// Classify a documentation site from its homepage.
pub fn detect(evidence: &Evidence<'_>) -> Detection {
    let html = evidence.html.to_lowercase();

    // mdBook first, unconditionally: a book *about* a generator is full of
    // that generator's name, and mdBook's own furniture is the only thing that
    // says what built the page rather than what it is about.
    if html.contains(RUSTDOC_BOOK) {
        return Detection {
            platform: Platform::MdBook,
            confidence: 0.95,
        };
    }

    for rule in RULES {
        if rule.markers.iter().any(|marker| html.contains(marker)) {
            return Detection {
                platform: rule.platform,
                confidence: rule.confidence,
            };
        }
    }

    Detection {
        platform: Platform::Generic,
        // Deliberately below `AUTO_ACCEPT`. "Nothing matched" is not the same
        // as "this is definitely a generic site", and the difference is
        // whether a user gets asked. P2-020 records `(Generic, 1.0)` as the
        // defect this avoids.
        confidence: 0.4,
    }
}

/// Fetch a site's homepage and classify it.
///
/// **One request.** P2-014 lists `book.toml` and `searchindex.js` as evidence,
/// and probing for them costs a round trip each; measured against S2-9's
/// corpus they add nothing, because every platform there is separable from the
/// homepage alone except two Sphinx sites that carry no marker anywhere — and
/// a probe would not find those either, since the files are not at the root.
///
/// The fetch goes through the ordinary [`Fetcher`](crate::fetch::Fetcher), so
/// `robots.txt`, the rate limiter and the SSRF guard all apply. Detection is
/// the *first* thing Tome does to a site a user names, which makes it exactly
/// the wrong place to skip any of them.
pub fn detect_site(
    fetcher: &crate::fetch::Fetcher,
    url: &url::Url,
) -> crate::error::Result<Detection> {
    /// A homepage large enough to hold any generator's markers. Beyond this a
    /// page is a single-file bundle, and reading more of it would not change
    /// the answer.
    const MAX_BODY: u64 = 2 * 1024 * 1024;

    let fetched = match fetcher.fetch(url, MAX_BODY, None)? {
        crate::fetch::FetchOutcome::Fetched(fetched) => fetched,
        // Only possible with validators, which this never sends.
        crate::fetch::FetchOutcome::NotModified => {
            return Ok(Detection {
                platform: Platform::Generic,
                confidence: 0.0,
            })
        }
    };

    let mut headers = BTreeMap::new();
    if let Some(content_type) = &fetched.content_type {
        headers.insert("content-type".to_owned(), content_type.clone());
    }

    // Lossy rather than strict: a page whose bytes are not valid UTF-8 still
    // has ASCII asset paths in it, and refusing to look would turn a mis-declared
    // charset into "this site cannot be detected".
    let html = String::from_utf8_lossy(&fetched.body);

    Ok(detect(&Evidence {
        url: fetched.final_url.as_str(),
        headers: &headers,
        html: &html,
    }))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn evidence<'a>(html: &'a str, headers: &'a BTreeMap<String, String>) -> Evidence<'a> {
        Evidence {
            url: "https://example.com/",
            headers,
            html,
        }
    }

    #[test]
    fn platform_names_round_trip() {
        for platform in Platform::ALL {
            assert_eq!(Platform::parse(platform.as_str()), Some(platform));
        }
    }

    #[test]
    fn an_unknown_label_is_not_silently_generic() {
        // A misspelt fixture label scored as `Generic` would inflate the
        // corpus's non-documentation count and make the detector look better
        // the more typos it contained.
        assert_eq!(Platform::parse("sphynx"), None);
        assert_eq!(Platform::parse(""), None);
        assert_eq!(Platform::parse("Generic"), None);
    }

    #[test]
    fn the_fallback_is_never_confident() {
        // The defect P2-020 names: `(Generic, 1.0)` makes "no idea"
        // indistinguishable from "certain".
        let headers = BTreeMap::new();
        let detection = detect(&evidence("<html></html>", &headers));
        assert_eq!(detection.platform, Platform::Generic);
        assert!(
            !detection.is_confident(),
            "the fallback must sit below AUTO_ACCEPT, got {}",
            detection.confidence
        );
    }

    #[test]
    fn the_rustdoc_book_is_mdbook_not_rustdoc() {
        // `doc.rust-lang.org/rustdoc/` is documentation *about* rustdoc, built
        // with mdBook. It contains the word "rustdoc" everywhere. P2-014's
        // sketch does `html.contains("rustdoc") -> Rustdoc`, and the corpus has
        // this page so the mistake cannot be made silently.
        let headers = BTreeMap::new();
        let html = r#"<html><head><title>The rustdoc book</title></head>
            <body><nav id="sidebar"><div class="sidebar-scrollbox">
            <a href="what-is-rustdoc.html">What is rustdoc?</a></div></nav>
            <main><p>rustdoc rustdoc rustdoc</p></main></body></html>"#;
        assert_eq!(detect(&evidence(html, &headers)).platform, Platform::MdBook);
    }

    #[test]
    fn a_mkdocs_site_on_readthedocs_is_not_sphinx() {
        // P2-014's first sketched rule is
        // `url.contains("readthedocs") -> ReadTheDocs, 0.95`. The corpus
        // contains a counter-example: RTD hosts MkDocs projects too. The
        // domain says who serves the pages, not what made them.
        let headers = BTreeMap::new();
        let html = r#"<html><head><link href="/assets/stylesheets/main.css"></head>
            <body class="md-container"><header class="md-header"
            data-md-component="header"></header></body></html>"#;
        let detection = detect(&Evidence {
            url: "https://mkdocs-macros-plugin.readthedocs.io/en/latest/",
            headers: &headers,
            html,
        });
        assert_eq!(detection.platform, Platform::MkDocs);
    }

    #[test]
    fn a_heavily_themed_sphinx_site_is_admitted_rather_than_guessed() {
        // Django's documentation is Sphinx-built and post-processed into a
        // template that carries no Sphinx marker at all — the "heavily themed
        // Sphinx" case P2-020 asks for. The right answer from a homepage alone
        // is "I do not know", which is what `Generic` below AUTO_ACCEPT means.
        // Getting this one *right* would mean a rule that fired on prose.
        let headers = BTreeMap::new();
        let html = r#"<html><head><title>Django documentation</title>
            <link rel="stylesheet" href="/s/css/djangoproject.css"></head>
            <body><div id="billboard"></div></body></html>"#;
        let detection = detect(&evidence(html, &headers));
        assert_eq!(detection.platform, Platform::Generic);
        assert!(!detection.is_confident());
    }

    #[test]
    fn a_marketing_page_is_never_confidently_a_documentation_platform() {
        // P2-020's headline metric. Crawling a company homepage with the
        // Sphinx scraper fills a library with mis-parsed pages that look fine
        // until they are read.
        let headers = BTreeMap::new();
        for html in [
            "<html><head><title>Acme Corp</title></head><body><h1>Ship faster</h1></body></html>",
            "<html><body>We are hiring! Read our documentation.</body></html>",
            "",
        ] {
            let detection = detect(&evidence(html, &headers));
            assert_eq!(detection.platform, Platform::Generic, "{html:?}");
            assert!(!detection.is_confident(), "{html:?}");
        }
    }

    #[test]
    fn detection_is_case_insensitive() {
        // Generators do not agree on the case of their own asset paths, and a
        // site behind a rewriting proxy may differ again.
        let headers = BTreeMap::new();
        for html in [
            "<script src=\"_static/DOCUMENTATION_OPTIONS.JS\"></script>",
            "<script src=\"_static/documentation_options.js\"></script>",
        ] {
            assert_eq!(detect(&evidence(html, &headers)).platform, Platform::Sphinx);
        }
    }

    #[test]
    fn confidence_stays_in_range() {
        let headers = BTreeMap::new();
        for html in ["", "<html></html>", "not html at all"] {
            let detection = detect(&evidence(html, &headers));
            assert!(
                (0.0..=1.0).contains(&detection.confidence),
                "{html:?} gave {}",
                detection.confidence
            );
        }
    }
}
