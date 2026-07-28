//! The reader's backend: page loading, rendering, and asset serving (S1-13).
//!
//! Three surfaces, and the third is the security-relevant one.
//!
//! # Commands
//!
//! [`list_sources`], [`list_pages`], and [`read_page`] are what the UI calls.
//! `read_page` does the work: read the stored AST from disk, render it to
//! HTML, and hand back the HTML plus the page outline. **Rendering happens on
//! every page view rather than being cached**, which SPIKE-002's numbers make
//! affordable (a 500 KB page crosses the IPC boundary in ~14 ms in a debug
//! build) and which is what lets a stylesheet or highlighter change take
//! effect without re-crawling anything.
//!
//! # The `tome://` asset protocol
//!
//! Localized assets live in `~/Library/Caches/Tome/data/<source>/assets/`,
//! outside the app bundle, so the webview cannot reach them by URL. A custom
//! URI scheme handler serves them, and it is the one place in the app where a
//! string from rendered page content becomes a filesystem path. It therefore
//! validates rather than trusts:
//!
//! - the source segment must parse as a [`SourceId`], which admits no
//!   separator, no dot-leading name, and no NUL;
//! - the second segment must be exactly `assets`;
//! - the filename must be a single `[A-Za-z0-9._-]` component;
//! - the resolved path is canonicalised and must still be inside the source's
//!   asset directory.
//!
//! Any one of those would probably do. All four are there because this is the
//! boundary where a traversal would read a file the user never asked to
//! share, and the renderer's own `local_asset_ref` check (which produces
//! these URLs) is not a guarantee the *handler* gets to rely on — a future
//! caller could construct one by hand.
//!
//! **Platform note:** on macOS a registered scheme is reached as
//! `tome://localhost/<path>`; Windows and Android use
//! `http://tome.localhost/<path>`. Tome ships macOS only (see `deny.toml`'s
//! target scoping and the PRD), so [`asset_base`] builds the macOS form. If
//! another platform is ever targeted, this is the function to change.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::http::{Request, Response, StatusCode};
use tome_core::db::Database;
use tome_core::highlight::Highlighter;
use tome_core::model::{PagePath, SourceId};
use tome_core::render::{render, OutlineEntry, RenderOptions};
use tome_core::store::PageStore;
use tome_core::Paths;

/// The URI scheme localized assets are served over.
pub const ASSET_SCHEME: &str = "tome";

/// Resolved once at startup and shared by every command.
pub struct ReaderState {
    pub paths: Paths,
}

/// One source, as the library sidebar needs it.
#[derive(Serialize)]
pub struct SourceSummary {
    id: String,
    name: String,
    category: String,
    page_count: u32,
    last_synced: Option<String>,
}

/// One page, as the page list needs it. Deliberately not the whole [`Page`]:
/// the content hash and the HTTP validators are sync machinery and have no
/// business crossing into the UI.
#[derive(Serialize)]
pub struct PageSummary {
    path: String,
    title: String,
}

/// A page, rendered and ready for the frame.
#[derive(Serialize)]
pub struct ReaderPage {
    source_id: String,
    path: String,
    title: String,
    /// The HTML for the sandboxed frame. Goes straight into one `postMessage`
    /// — SPIKE-002 measured 0.1 ms for 500 KB and found chunking *slower*.
    html: String,
    outline: Vec<OutlineEntry>,
}

/// Every source with a configuration on disk or a row in the database.
#[tauri::command]
pub fn list_sources(state: tauri::State<'_, ReaderState>) -> Result<Vec<SourceSummary>, String> {
    let database = Database::open(&state.paths).map_err(to_message)?;
    let sources = database.list_sources().map_err(to_message)?;
    Ok(sources
        .into_iter()
        .map(|source| SourceSummary {
            page_count: database.page_count(&source.id).unwrap_or(source.page_count),
            id: source.id.to_string(),
            name: source.name,
            category: source.category,
            last_synced: source.last_synced.map(|t| t.to_rfc3339()),
        })
        .collect())
}

/// Every page in one source, ordered by path so the list is stable between
/// launches. Ordering in the UI instead would sort a list that the backend
/// had already shuffled, and "stable" is the property that matters.
#[tauri::command]
pub fn list_pages(
    state: tauri::State<'_, ReaderState>,
    source_id: String,
) -> Result<Vec<PageSummary>, String> {
    let source = SourceId::new(source_id).map_err(to_message)?;
    let database = Database::open(&state.paths).map_err(to_message)?;
    let mut pages = database.list_pages(&source).map_err(to_message)?;
    pages.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str()));
    Ok(pages
        .into_iter()
        .map(|page| PageSummary {
            path: page.path.to_string(),
            title: page.title,
        })
        .collect())
}

/// Load one page and render it.
#[tauri::command]
pub fn read_page(
    state: tauri::State<'_, ReaderState>,
    source_id: String,
    path: String,
) -> Result<ReaderPage, String> {
    let source = SourceId::new(source_id).map_err(to_message)?;
    let page_path = PagePath::new(path).map_err(to_message)?;

    let store = PageStore::new(&state.paths, &source);
    let stored = store
        .read(&page_path)
        .map_err(to_message)?
        // Not an error type: a page that is not in the library is an ordinary
        // outcome (a stale bookmark, a link to a page the crawl never
        // reached), and the UI shows it as a message rather than a failure.
        .ok_or_else(|| format!("This page is not in the library: {page_path}"))?;

    let base = asset_base(&source);
    let rendered = render(
        &stored.body,
        &RenderOptions {
            asset_base: &base,
            highlighter: Highlighter::shared(),
        },
    );

    Ok(ReaderPage {
        source_id: source.to_string(),
        path: page_path.to_string(),
        // The renderer's title (the page's own h1) is better than the stored
        // one when they differ; fall back when a page has no h1 at all.
        title: rendered.title.unwrap_or(stored.title),
        html: rendered.html,
        outline: rendered.outline,
    })
}

/// The URL prefix a source's assets are served under.
pub fn asset_base(source: &SourceId) -> String {
    format!("{ASSET_SCHEME}://localhost/{source}/")
}

/// Serve one localized asset. See the module docs for why this validates four
/// separate things.
pub fn serve_asset(paths: &Paths, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    match resolve_asset(paths, request.uri().path()) {
        Some(file) => match std::fs::read(&file) {
            Ok(bytes) => Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", content_type(&file))
                // The bytes are content-addressed: the name IS the hash, so
                // the content behind a given URL can never change.
                .header("Cache-Control", "public, max-age=31536000, immutable")
                // Defence in depth against a stored SVG or HTML-ish asset
                // being sniffed into something scriptable. SVG sanitization
                // is a flagged S1-10 follow-up; this narrows the window.
                .header("X-Content-Type-Options", "nosniff")
                .body(bytes)
                .unwrap_or_else(|_| empty(StatusCode::INTERNAL_SERVER_ERROR)),
            Err(_) => empty(StatusCode::NOT_FOUND),
        },
        None => empty(StatusCode::NOT_FOUND),
    }
}

/// `/<source-id>/assets/<filename>` → a real file inside that source's asset
/// directory, or `None`.
fn resolve_asset(paths: &Paths, uri_path: &str) -> Option<PathBuf> {
    let decoded = percent_decode(uri_path);
    let mut segments = decoded.trim_start_matches('/').split('/');

    let source = SourceId::new(segments.next()?).ok()?;
    if segments.next()? != "assets" {
        return None;
    }
    let filename = segments.next()?;
    // Exactly three segments: a fourth means someone is building a deeper
    // path than this scheme has.
    if segments.next().is_some() {
        return None;
    }
    if filename.is_empty()
        || !filename
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        return None;
    }

    let dir = paths.assets_dir(&source);
    let candidate = dir.join(filename);

    // Canonicalise both sides and re-check containment. The charset check
    // above already makes `..` unrepresentable; this catches the case it
    // cannot see — a symlink inside the asset directory pointing elsewhere.
    let real = candidate.canonicalize().ok()?;
    let real_dir = dir.canonicalize().ok()?;
    real.starts_with(&real_dir).then_some(real)
}

/// Percent-decoding, because the handler receives the URI as written and a
/// filename is a hash with an extension — `%2e%2e%2f` must be decoded before
/// the traversal check, never after. (The fixture server learned the same
/// lesson; see `tome-testkit`.)
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(byte) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Content type from the extension. The extension came from the *sniffed*
/// content type when the asset was stored (S1-10), not from the URL, so this
/// is round-tripping the earlier decision rather than trusting a filename.
fn content_type(file: &Path) -> &'static str {
    match file
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        // Served as an image, never as a document: `image/svg+xml` in an
        // `<img>` cannot run script, but the same bytes at a top-level URL
        // can. `nosniff` above plus the frame's `img-src` keep it to that.
        Some("svg") => "image/svg+xml",
        // Deliberately not `application/octet-stream`: an unknown type should
        // not tempt the webview into sniffing. Nothing legitimate reaches it.
        _ => "application/binary",
    }
}

fn empty(status: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .body(Vec::new())
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

/// Errors cross the IPC boundary as strings. `tome_core::Error`'s messages
/// are already written to be user-facing and to carry no reading history.
fn to_message(error: tome_core::Error) -> String {
    match error.suggestion() {
        Some(hint) => format!("{error} {hint}"),
        None => error.to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn library() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under_root(dir.path());
        paths.ensure_created().unwrap();
        (dir, paths)
    }

    fn with_asset(paths: &Paths, source: &str, filename: &str) -> SourceId {
        let source = SourceId::new(source).unwrap();
        paths.ensure_source_dirs(&source).unwrap();
        std::fs::write(paths.assets_dir(&source).join(filename), b"bytes").unwrap();
        source
    }

    #[test]
    fn serves_an_asset_that_exists() {
        let (_dir, paths) = library();
        with_asset(&paths, "demo", "abc123.png");
        let file = resolve_asset(&paths, "/demo/assets/abc123.png").expect("resolves");
        assert!(file.ends_with("abc123.png"));
    }

    #[test]
    fn refuses_every_shape_of_traversal() {
        let (_dir, paths) = library();
        with_asset(&paths, "demo", "abc123.png");
        // A file the handler must never serve, one directory up from assets.
        let secret = paths.source_data_dir(&SourceId::new("demo").unwrap());
        std::fs::write(secret.join("secret.txt"), b"private").unwrap();

        for path in [
            "/demo/assets/../secret.txt",
            // Percent-encoded, the standard way that check gets defeated
            // when decoding happens after the check instead of before.
            "/demo/assets/%2e%2e%2fsecret.txt",
            "/demo/assets/%2E%2E/secret.txt",
            "/demo/../demo/assets/abc123.png",
            "/demo/raw/page.html",
            "/demo/assets/sub/abc123.png",
            "/demo/assets/",
            "/demo/assets",
            "/../../etc/passwd",
            "/demo/assets/abc123.png/extra",
        ] {
            assert!(
                resolve_asset(&paths, path).is_none(),
                "{path} should not resolve"
            );
        }
    }

    #[test]
    fn refuses_a_source_id_that_is_not_one() {
        let (_dir, paths) = library();
        with_asset(&paths, "demo", "abc123.png");
        for source in ["..", ".hidden", "a/b", "", "a b"] {
            let path = format!("/{source}/assets/abc123.png");
            assert!(resolve_asset(&paths, &path).is_none(), "{path}");
        }
    }

    #[test]
    fn refuses_a_symlink_that_escapes_the_asset_directory() {
        // The one case the charset check cannot see. `..` is unrepresentable
        // in a filename, but a symlink inside the directory is a path the
        // filesystem resolves for us -- hence the canonicalise-and-compare.
        let (_dir, paths) = library();
        let source = with_asset(&paths, "demo", "abc123.png");
        let outside = paths.state_root().join("outside.txt");
        std::fs::write(&outside, b"private").unwrap();
        let link = paths.assets_dir(&source).join("link.png");
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        assert!(resolve_asset(&paths, "/demo/assets/link.png").is_none());
    }

    #[test]
    fn a_missing_asset_is_a_404_not_a_crash() {
        let (_dir, paths) = library();
        with_asset(&paths, "demo", "abc123.png");
        assert!(resolve_asset(&paths, "/demo/assets/nothere.png").is_none());
    }

    #[test]
    fn the_asset_base_matches_what_the_handler_parses() {
        // These two are a matched pair: the renderer builds URLs with
        // `asset_base` and the handler takes them apart. A change to either
        // without the other produces a page of broken images.
        let source = SourceId::new("rust-std").unwrap();
        let base = asset_base(&source);
        assert_eq!(base, "tome://localhost/rust-std/");

        let (_dir, paths) = library();
        with_asset(&paths, "rust-std", "deadbeef.svg");
        let url = format!("{base}assets/deadbeef.svg");
        let uri_path = url.trim_start_matches("tome://localhost");
        assert!(resolve_asset(&paths, uri_path).is_some(), "{url}");
    }

    #[test]
    fn content_types_come_from_the_stored_extension() {
        assert_eq!(content_type(Path::new("a/b.png")), "image/png");
        assert_eq!(content_type(Path::new("a/b.SVG")), "image/svg+xml");
        assert_eq!(content_type(Path::new("a/b.exe")), "application/binary");
        assert_eq!(content_type(Path::new("a/b")), "application/binary");
    }

    #[test]
    fn percent_decoding_handles_the_awkward_cases() {
        assert_eq!(percent_decode("/a%2Fb"), "/a/b");
        assert_eq!(percent_decode("/a%2"), "/a%2");
        assert_eq!(percent_decode("/a%zz"), "/a%zz");
        assert_eq!(percent_decode("/plain"), "/plain");
    }
}
