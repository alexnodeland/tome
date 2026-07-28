//! [`DocSet`]: what a scrape produces, before anything is persisted.

use serde::{Deserialize, Serialize};

use crate::model::{Node, Page, PagePath};

/// One entry in a source's navigation tree. The tree comes from the
/// platform's own structure where one exists (Sphinx `searchindex.js`,
/// mdBook `SUMMARY.md`, rustdoc's module hierarchy) and from crawl order
/// where none does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TocEntry {
    pub title: String,
    /// The page this entry opens, or `None` for a grouping node that only
    /// exists to hold children (a part title in a book, a module with no
    /// index page).
    pub path: Option<PagePath>,
    /// Fragment within the page (`#section`), for platforms whose TOC is
    /// finer-grained than their pages.
    pub fragment: Option<String>,
    pub children: Vec<TocEntry>,
}

impl TocEntry {
    pub fn new(title: impl Into<String>, path: Option<PagePath>) -> Self {
        Self {
            title: title.into(),
            path,
            fragment: None,
            children: Vec::new(),
        }
    }
}

/// One page as scraped: its metadata and its normalized body. The pair stays
/// together through the pipeline; persistence splits it (metadata to the
/// database, body to the page store) and [`Page`] documents why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocPage {
    pub meta: Page,
    /// Always a [`Node::Document`] root.
    pub body: Node,
}

/// The result of scraping a source: every page fetched plus the navigation
/// tree. This is a value, not a database view — it exists so the pipeline
/// stages (fetch → parse → normalize → localize → persist) hand one thing to
/// each other and so a test can assert on a whole scrape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocSet {
    pub pages: Vec<DocPage>,
    pub toc: Vec<TocEntry>,
}

impl DocSet {
    pub fn new(pages: Vec<DocPage>, toc: Vec<TocEntry>) -> Self {
        Self { pages, toc }
    }
}
