//! The page store: normalized page content on disk (S1-13).
//!
//! Page *metadata* lives in SQLite (`db.rs`) because it is loaded in bulk —
//! for the sidebar, for search results, for `tome list`. Page *content* lives
//! here, one file per page, because it is loaded one page at a time and a
//! 5 000-page source would otherwise put tens of megabytes of AST into a
//! database that is queried on every keystroke.
//!
//! What is stored is the **AST, not rendered HTML**. Rendering is cheap
//! (S1-13 measures in single-digit milliseconds) and the AST is the semantic
//! model: a change to the renderer, the stylesheet, or the highlighter takes
//! effect on the next page view rather than requiring a re-crawl of every
//! source. The frozen serde shape (`model/node.rs`) is what makes that safe.
//!
//! # Why files are named by hash and not by path
//!
//! A [`PagePath`] cannot traverse — no separators-with-dots, no `..`, no
//! leading `/` — so joining it under `pages_dir` is already contained. The
//! reason it is *hashed* instead is **macOS**: the default APFS volume is
//! case-insensitive, so `Tutorial.html` and `tutorial.html` are two pages
//! upstream and one file here. Silently collapsing two pages into one is the
//! kind of bug that shows up as "some pages show the wrong content" months
//! later. Hashing the path sidesteps it, and also flattens the directory (a
//! deep documentation tree becomes one directory of files, which is faster to
//! enumerate and impossible to get wrong).
//!
//! The path is stored *inside* each file so the directory stays
//! self-describing — `grep -l` still finds a page by its URL path.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::hash::{hex, sha256};
use crate::model::{Node, PagePath, SourceId};
use crate::Paths;

/// One page's content as stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredPage {
    /// The page's own path, so a file on disk identifies itself.
    pub path: PagePath,
    pub title: String,
    pub description: Option<String>,
    /// Always a [`Node::Document`] root. Normalized, sanitized, and
    /// asset-localized — everything the renderer needs and nothing it has to
    /// re-derive.
    pub body: Node,
}

/// Reads and writes page content for one source.
pub struct PageStore {
    dir: PathBuf,
}

impl PageStore {
    /// The store for `source`. Does not create anything; [`write`](Self::write)
    /// does, so a read-only consumer never causes a directory to appear.
    pub fn new(paths: &Paths, source: &SourceId) -> Self {
        Self {
            dir: paths.pages_dir(source),
        }
    }

    /// The store rooted at an arbitrary directory. For tests and for the
    /// export path; ordinary callers use [`new`](Self::new) so that the
    /// location comes from `paths` and nowhere else.
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where one page's file lives.
    pub fn file(&self, path: &PagePath) -> PathBuf {
        self.dir
            .join(format!("{}.json", hex(&sha256(path.as_str().as_bytes()))))
    }

    /// Write a page, creating the directory if needed.
    ///
    /// Writes to a temporary file and renames, so a crash mid-write leaves
    /// the previous version rather than a truncated one. A half-written page
    /// would fail to parse on the next read, and the reader would report a
    /// corrupt library for what was really an interrupted sync.
    pub fn write(&self, page: &StoredPage) -> Result<()> {
        fs::create_dir_all(&self.dir).map_err(|source| Error::CreateDirectory {
            path: self.dir.clone(),
            source,
        })?;
        let final_path = self.file(&page.path);
        let temp_path = final_path.with_extension("json.tmp");

        let json = serde_json::to_vec(page).map_err(|e| Error::PageStore {
            message: format!("could not serialize page: {e}"),
        })?;
        fs::write(&temp_path, &json)?;
        fs::rename(&temp_path, &final_path)?;
        Ok(())
    }

    /// Read a page. `Ok(None)` when the source has not been pulled, or the
    /// page is not part of it — a missing page is an ordinary state, not an
    /// error.
    pub fn read(&self, path: &PagePath) -> Result<Option<StoredPage>> {
        let file = self.file(path);
        let bytes = match fs::read(&file) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let page: StoredPage = serde_json::from_slice(&bytes).map_err(|e| Error::PageStore {
            // The FILE, not the page path. The file name is a content hash
            // and reveals nothing about what was being read; the page path
            // is reading history, and `error.rs` rule 2 keeps it out of
            // messages that end up in logs and pasted issues.
            message: format!("{} ({e})", file.display()),
        })?;
        Ok(Some(page))
    }

    /// Every page path this store holds, in no particular order.
    ///
    /// Reads each file, because the *name* is a hash and carries no path.
    /// Callers that want an ordered or filtered list should use the database
    /// instead; this exists for repair and export, where the files are the
    /// only source of truth left.
    pub fn list(&self) -> Result<Vec<PagePath>> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut paths = Vec::new();
        for entry in entries.flatten() {
            let file = entry.path();
            if file.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            if let Ok(bytes) = fs::read(&file) {
                if let Ok(page) = serde_json::from_slice::<StoredPage>(&bytes) {
                    paths.push(page.path);
                }
            }
        }
        Ok(paths)
    }

    /// Remove one page's file. `Ok(false)` if it was not there.
    pub fn remove(&self, path: &PagePath) -> Result<bool> {
        match fs::remove_file(self.file(path)) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn page(path: &str, title: &str) -> StoredPage {
        StoredPage {
            path: PagePath::new(path).unwrap(),
            title: title.to_owned(),
            description: None,
            body: Node::Document {
                children: vec![Node::Paragraph {
                    children: vec![Node::Text {
                        value: title.to_owned(),
                    }],
                }],
            },
        }
    }

    #[test]
    fn round_trips_a_page() {
        let dir = tempfile::tempdir().unwrap();
        let store = PageStore::at(dir.path());
        let original = page("api/reference.html", "Reference");

        store.write(&original).unwrap();
        let read = store.read(&original.path).unwrap().unwrap();
        assert_eq!(read, original);
    }

    #[test]
    fn a_missing_page_is_none_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = PageStore::at(dir.path().join("never-created"));
        assert_eq!(store.read(&PagePath::new("x.html").unwrap()).unwrap(), None);
        assert_eq!(store.list().unwrap(), Vec::new());
    }

    #[test]
    fn paths_differing_only_in_case_are_different_pages() {
        // The reason files are named by hash. On a default macOS volume,
        // naming them by path would put both of these in one file and the
        // second write would silently overwrite the first.
        let dir = tempfile::tempdir().unwrap();
        let store = PageStore::at(dir.path());
        let lower = page("tutorial.html", "lower");
        let upper = page("Tutorial.html", "upper");

        store.write(&lower).unwrap();
        store.write(&upper).unwrap();

        assert_ne!(store.file(&lower.path), store.file(&upper.path));
        assert_eq!(store.read(&lower.path).unwrap().unwrap().title, "lower");
        assert_eq!(store.read(&upper.path).unwrap().unwrap().title, "upper");
    }

    #[test]
    fn a_deep_path_stays_one_flat_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = PageStore::at(dir.path());
        let deep = page("a/b/c/d/e/page.html", "Deep");
        store.write(&deep).unwrap();

        let files: Vec<_> = fs::read_dir(dir.path()).unwrap().flatten().collect();
        assert_eq!(files.len(), 1);
        assert!(files[0].path().is_file());
    }

    #[test]
    fn list_reports_paths_not_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let store = PageStore::at(dir.path());
        store.write(&page("a.html", "A")).unwrap();
        store.write(&page("b/c.html", "C")).unwrap();

        let mut listed = store.list().unwrap();
        listed.sort();
        assert_eq!(
            listed.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
            ["a.html", "b/c.html"]
        );
    }

    #[test]
    fn a_write_leaves_no_temporary_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        let store = PageStore::at(dir.path());
        store.write(&page("a.html", "A")).unwrap();
        for entry in fs::read_dir(dir.path()).unwrap().flatten() {
            assert!(
                !entry.path().to_string_lossy().ends_with(".tmp"),
                "{entry:?}"
            );
        }
    }

    #[test]
    fn a_corrupt_file_reports_the_file_and_not_the_page_path() {
        // `error.rs` rule 2: what someone reads never reaches an error
        // message. The file name is a content hash, which is safe to print
        // and is what someone would actually need in order to delete it.
        let dir = tempfile::tempdir().unwrap();
        let store = PageStore::at(dir.path());
        let path = PagePath::new("api/reference.html").unwrap();
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(store.file(&path), b"{not json").unwrap();

        let message = store.read(&path).unwrap_err().to_string();
        assert!(!message.contains("api/reference.html"), "{message}");
        assert!(message.contains(".json"), "{message}");
        assert!(message.contains("Re-sync"), "{message}");
    }

    #[test]
    fn overwriting_replaces_rather_than_appending() {
        let dir = tempfile::tempdir().unwrap();
        let store = PageStore::at(dir.path());
        store.write(&page("a.html", "first")).unwrap();
        store.write(&page("a.html", "second")).unwrap();
        let read = store
            .read(&PagePath::new("a.html").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(read.title, "second");
    }

    #[test]
    fn remove_reports_whether_there_was_anything_to_remove() {
        let dir = tempfile::tempdir().unwrap();
        let store = PageStore::at(dir.path());
        let path = PagePath::new("a.html").unwrap();
        store.write(&page("a.html", "A")).unwrap();
        assert!(store.remove(&path).unwrap());
        assert!(!store.remove(&path).unwrap());
    }
}
