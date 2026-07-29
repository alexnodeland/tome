//! Documentation platform detection (S2-9 harness, S2-10 detector; P2-014).
//!
//! # What this is, at this commit
//!
//! **A deliberately weak baseline.** It classifies everything as
//! [`Platform::Generic`] at low confidence. That is not a placeholder left by
//! accident — it is the thing S2-9's corpus is built to measure, so that
//! S2-10's real detector has a number to beat and a confusion matrix to
//! explain it.
//!
//! S2-1 established the order and it is not negotiable here either: every
//! measured finding in Stage 2 came from a harness that existed *before* the
//! code it scored — the query-parser defect, the code-block symbol defect, the
//! all-declarations ranking regression. Each was invisible to inspection and
//! obvious to a corpus.
//!
//! # Detection must be allowed to say "I don't know"
//!
//! P2-020 records the original detector as returning `(Generic, 1.0)` — full
//! confidence in the fallback — which makes "I have no idea" indistinguishable
//! from "I am certain". [`Detection::confidence`] below
//! [`AUTO_ACCEPT`] means the UI asks rather than assumes, and the fallback
//! sits well under it.
//!
//! The corpus contains marketing pages that are not documentation at all, and
//! the success metric that matters most is **no confident-but-wrong
//! classification of a non-doc site**: guessing Sphinx for a company homepage
//! and crawling it as documentation is worse than admitting uncertainty.

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

/// Classify a documentation site from its homepage.
///
/// **Baseline only at this commit** — see the module docs. S2-10 replaces the
/// body; the signature, the corpus and the harness are what S2-9 delivers.
pub fn detect(evidence: &Evidence<'_>) -> Detection {
    let _ = evidence;
    Detection {
        platform: Platform::Generic,
        // Well below `AUTO_ACCEPT`, so the fallback is never mistaken for an
        // answer. P2-020 records `(Generic, 1.0)` as the defect this avoids.
        confidence: 0.1,
    }
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
