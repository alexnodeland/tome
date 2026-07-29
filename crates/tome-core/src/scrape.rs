//! Per-platform scraping knowledge (S2-11, specs P2-010/011/012).
//!
//! # How little is left here, and why
//!
//! The plan budgets four scrapers at 1–2 weeks each. Most of that work has
//! already happened, in the *generic* path: S1-8's furniture rules
//! ([`crate::parse::DROP_CLASS_FRAGMENTS`]) were each added because something
//! leaked into the golden corpus, and that corpus spans Sphinx, rustdoc,
//! mdBook, Node, Hugo and go.dev. Rules developed against six platforms are
//! not "generic" by accident — they are cross-platform because they were
//! measured across platforms.
//!
//! Re-measured before writing this module: of 26 golden pages, exactly **one**
//! still carried platform furniture — rustdoc's `1.0.0 · Source` sub-heading.
//! Titles were already clean (normalization takes them from the `h1`, not from
//! `<title>`, so `std::vec - Rust` never existed as a title). Content roots
//! were already right, because Sphinx sets `role="main"` and both rustdoc and
//! mdBook use `<main>`, all of which the generic root selection already tries.
//!
//! So what remains is the part the generic path *cannot* safely do.
//!
//! # Exact tokens, not substrings
//!
//! The generic list matches class substrings, because it has to: every
//! generator compounds words differently and it cannot know which. That
//! bluntness is affordable there only because each entry earned its place by
//! fixing a visible leak.
//!
//! A profile knows which generator it is looking at, so it matches **exact
//! class tokens**. `src` as a substring would hit `srcset`, `source-code` and
//! `bootstrap-container`; as an exact token on a page known to be rustdoc it
//! is precisely the source-link furniture and nothing else. That is the whole
//! difference, and it is why these rules live here rather than being added to
//! the generic list.
//!
//! # What is measured, and what is not
//!
//! Of the three profiles, **only rustdoc's changes anything on the golden
//! corpus.** Adding all three moved four files, all rustdoc, removing exactly
//! the `1.0.0 · Source` sub-heading and nothing else. Sphinx's and mdBook's
//! rules are therefore **unmeasured**: the corpus's Sphinx pages carry no
//! `viewcode-link` or ReadTheDocs version flyout, and mdBook's heading
//! permalinks were already unwrapped by the generic `unwrap_self_permalink`.
//!
//! They are kept rather than deleted because they encode real markup that
//! those generators emit, and the corpus simply does not contain an instance —
//! but "kept" is not "verified", and a reader should treat them as untested
//! until a page that exercises them is added.
//!
//! One rule was **removed** for cause, and it is the reason this module is
//! written as exact tokens with a corpus behind it: mdBook's `header` class is
//! on the anchor *inside* each heading, so dropping it deleted every heading
//! from every mdBook page. The golden diff showed it immediately.

use crate::model::SourceType;

/// Platform-specific scraping rules.
pub struct Profile {
    /// Exact class tokens whose element and subtree are furniture.
    ///
    /// Matched against whole `class` tokens, case-insensitively — never as
    /// substrings. See the module docs.
    pub drop_classes: &'static [&'static str],
}

/// Sphinx and ReadTheDocs (P2-010). One profile: ReadTheDocs *is* Sphinx,
/// hosted, and the markup a scraper reads is Sphinx's.
static SPHINX: Profile = Profile {
    drop_classes: &[
        // The `¶` that follows every heading. It is a permalink affordance,
        // and in a reader it is a stray pilcrow.
        "headerlink",
        // "[source]" links beside API entries, pointing at a code browser
        // that is not part of the documentation.
        "viewcode-link",
        "reference-download",
        // Left navigation and the "previous | next | index" strip. Usually
        // outside the content root, but themes move them inside it.
        "sphinxsidebar",
        "sphinxsidebarwrapper",
        "related",
        "sourcelink",
        // The "Edit on GitHub" / version flyout ReadTheDocs injects.
        "rst-versions",
        "rst-current-version",
        "wy-nav-side",
        "wy-breadcrumbs-aside",
    ],
};

/// rustdoc (P2-011).
static RUSTDOC: Profile = Profile {
    drop_classes: &[
        // The measured leak: `<span class="sub-heading"><span class="since">
        // 1.0.0</span> · <a class="src">Source</a></span>` renders in a reader
        // as a bare version number, a middot, and a word.
        "sub-heading",
        "src",
        "since",
        // The `§` permalink beside every item, rustdoc's equivalent of
        // Sphinx's headerlink.
        "anchor",
        // Toolbar affordances: "Copy item path", theme picker, the search
        // form's help. Not `toggle` — `<details class="toggle top-doc">` is
        // where the item's documentation lives.
        "copy-path",
        "rustdoc-toolbar",
        "help-button",
        "search-form",
        "out-of-band",
        "sidebar-elems",
    ],
};

/// mdBook (P2-012).
static MDBOOK: Profile = Profile {
    drop_classes: &[
        // The « » chapter arrows, duplicated for mobile. They appear twice on
        // every page and say only "Previous"/"Next".
        "nav-chapters",
        "mobile-nav-chapters",
        "nav-wrapper",
        // The top bar: sidebar toggle, theme picker, search, print.
        "icon-button",
        "menu-bar",
        "menu-title",
        "right-buttons",
        "left-buttons",
        // **Not `header`.** mdBook writes `<a class="header" href="#x">Text</a>`
        // *inside* the heading, so the heading's own text is the link's text —
        // dropping it deletes every heading on the page. The golden corpus
        // caught that on the first run, which is what the corpus is for. The
        // generic `unwrap_self_permalink` already unwraps this shape.
    ],
};

/// The profile for a source type, if it has one.
///
/// `None` means the generic path, unmodified — which is the right answer for
/// `Generic`, `Local` and `Docset`, and for `Man`, whose HTML comes from
/// `mandoc` rather than from a documentation generator.
pub fn profile_for(kind: SourceType) -> Option<&'static Profile> {
    match kind {
        SourceType::ReadTheDocs => Some(&SPHINX),
        SourceType::Rustdoc => Some(&RUSTDOC),
        SourceType::MdBook => Some(&MDBOOK),
        // Deliberately no catch-all: `SourceType` is `#[non_exhaustive]`, but
        // that binds only *other* crates, so inside `tome-core` this match must
        // stay exhaustive. A new platform then fails the build here and forces a
        // decision, rather than silently inheriting the generic path.
        SourceType::Man | SourceType::Generic | SourceType::Local | SourceType::Docset => None,
    }
}

impl Profile {
    /// Whether any of this element's class tokens is furniture.
    pub fn drops(&self, class_attribute: &str) -> bool {
        class_attribute.split_whitespace().any(|token| {
            self.drop_classes
                .iter()
                .any(|name| token.eq_ignore_ascii_case(name))
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn every_html_platform_has_a_profile() {
        assert!(profile_for(SourceType::ReadTheDocs).is_some());
        assert!(profile_for(SourceType::Rustdoc).is_some());
        assert!(profile_for(SourceType::MdBook).is_some());
    }

    #[test]
    fn the_generic_path_is_left_alone() {
        // A profile is knowledge about a specific generator. There is none to
        // have about "some website", and `Man` pages come from mandoc rather
        // than a documentation generator at all.
        for kind in [
            SourceType::Generic,
            SourceType::Local,
            SourceType::Docset,
            SourceType::Man,
        ] {
            assert!(profile_for(kind).is_none(), "{kind:?}");
        }
    }

    #[test]
    fn tokens_match_whole_classes_not_substrings() {
        // The entire reason these rules live here rather than in the generic
        // substring list. `src` as a substring would hit all of these.
        let rustdoc = profile_for(SourceType::Rustdoc).expect("rustdoc profile");
        assert!(rustdoc.drops("src"));
        assert!(rustdoc.drops("docblock src short"));
        assert!(!rustdoc.drops("srcset"));
        assert!(!rustdoc.drops("source-code"));
        assert!(!rustdoc.drops("bootstrap-container"));
    }

    #[test]
    fn matching_is_case_insensitive() {
        let sphinx = profile_for(SourceType::ReadTheDocs).expect("sphinx profile");
        assert!(sphinx.drops("HeaderLink"));
        assert!(sphinx.drops("headerlink"));
    }

    #[test]
    fn an_empty_or_absent_class_drops_nothing() {
        let mdbook = profile_for(SourceType::MdBook).expect("mdbook profile");
        assert!(!mdbook.drops(""));
        assert!(!mdbook.drops("   "));
    }

    #[test]
    fn rustdoc_keeps_the_element_its_documentation_lives_in() {
        // `<details class="toggle top-doc">` holds the item's docs. Dropping
        // anything called "toggle" would take the page's content with the
        // affordance, which is the failure mode this list is one typo from.
        let rustdoc = profile_for(SourceType::Rustdoc).expect("rustdoc profile");
        assert!(!rustdoc.drops("toggle top-doc"));
        assert!(!rustdoc.drops("toggle"));
        assert!(!rustdoc.drops("docblock"));
    }

    #[test]
    fn mdbook_does_not_drop_the_element_its_headings_live_in() {
        // The failure this profile actually hit. mdBook writes
        // `<a class="header" href="#x">Features</a>` *inside* the `<h2>`, so
        // the heading's text is the link's text: dropping `.header` deleted
        // every heading on every mdBook page in the golden corpus.
        let mdbook = profile_for(SourceType::MdBook).expect("mdbook profile");
        assert!(!mdbook.drops("header"));
        assert!(!mdbook.drops("page-header-title"));
    }
}
