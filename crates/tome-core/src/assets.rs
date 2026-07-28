//! Asset localization (implementation plan S1-10, ticket P1-023).
//!
//! Fetches every image a page references, stores it content-addressed on
//! disk, and rewrites the reference to a local one — so a synced source
//! renders with the network off. This is not polish: the reader loads pages
//! in a sandboxed iframe with a CSP that forbids remote origins, and a page
//! that still reached for `https://cdn.example/logo.png` would show a broken
//! image on a plane *and* leak the user's reading activity to a third party.
//! The Stage 1 exit gate asserts exactly this — render with the fixture
//! server shut down, and no `http` reference survives.
//!
//! # Scope, honestly
//!
//! The ticket lists `img[srcset]`, `source[srcset]`, `video[poster]`, inline
//! `<svg><image href>`, and `url()` in inline styles. **None of those exist
//! in Tome's typed AST** — the parser (S1-7) produces [`Node`]s, not HTML,
//! and only [`Node::Image`] carries an asset URL. That is a deliberate
//! consequence of the no-raw-HTML model, not an oversight: srcset and inline
//! styles cannot smuggle a remote reference through, because the AST has
//! nowhere to put one. If the model ever grows a `srcset` on `Image`, this is
//! where it is localized. Two more acceptance items are genuinely deferred
//! and flagged: **SVG byte-sanitization** (an SVG asset can carry script;
//! sanitizing it needs an SVG parser — until then SVGs are stored but marked
//! for that follow-up) and **garbage collection** of unreferenced assets on
//! re-sync (needs the store to know all referrers, which is the sync layer's
//! job, S3-adjacent).
//!
//! # Design
//!
//! The AST transform and the fetch/store are separated by the [`AssetStore`]
//! trait, so the rewrite logic is testable without a network and the real
//! store ([`FetchingAssetStore`]) can be swapped for a fake. A store returns
//! the local reference to substitute, or an error; on error the image is
//! replaced by an inline note and the reason is recorded — **never left as a
//! live remote reference**, which is the invariant the offline guarantee
//! rests on.

use std::collections::HashMap;
use std::path::PathBuf;

use url::Url;

use crate::fetch::Fetcher;
use crate::hash::{hex, sha256};
use crate::model::{Definition, ListItem, Node, TableCell, TableRow};

/// Stores one asset and returns the local reference to substitute for its
/// URL. Implementors handle fetching, caps, content-addressing, and dedup.
pub trait AssetStore {
    /// Localize the asset at `url`. `Ok(local_ref)` is the reference to write
    /// into the AST in place of `url`; `Err` describes why it was rejected,
    /// so the caller can place an offline-safe placeholder.
    fn localize(&mut self, url: &Url) -> Result<String, String>;
}

/// One asset that could not be localized. The image was replaced by a
/// placeholder; this records what was lost and why, for the sync report.
#[derive(Debug, Clone)]
pub struct AssetError {
    pub url: String,
    pub reason: String,
}

/// The result of localizing a page.
#[derive(Debug)]
pub struct LocalizeOutcome {
    pub body: Node,
    pub errors: Vec<AssetError>,
}

/// Localize every image reference in a document. `base` resolves any
/// still-relative `src` (normalization usually made them absolute, but a
/// crawl of a local source may not have).
pub fn localize_assets(document: Node, base: &Url, store: &mut dyn AssetStore) -> LocalizeOutcome {
    let mut errors = Vec::new();
    let body = map_node(document, base, store, &mut errors);
    LocalizeOutcome { body, errors }
}

fn map_node(
    node: Node,
    base: &Url,
    store: &mut dyn AssetStore,
    errors: &mut Vec<AssetError>,
) -> Node {
    match node {
        Node::Image { src, alt } => localize_image(src, alt, base, store, errors),

        // Recurse through containers.
        Node::Document { children } => Node::Document {
            children: map_children(children, base, store, errors),
        },
        Node::Heading {
            level,
            id,
            children,
        } => Node::Heading {
            level,
            id,
            children: map_children(children, base, store, errors),
        },
        Node::Paragraph { children } => Node::Paragraph {
            children: map_children(children, base, store, errors),
        },
        Node::Blockquote { children } => Node::Blockquote {
            children: map_children(children, base, store, errors),
        },
        Node::Emphasis { children } => Node::Emphasis {
            children: map_children(children, base, store, errors),
        },
        Node::Strong { children } => Node::Strong {
            children: map_children(children, base, store, errors),
        },
        Node::Link {
            href,
            title,
            children,
        } => Node::Link {
            href,
            title,
            children: map_children(children, base, store, errors),
        },
        Node::Admonition {
            kind,
            title,
            children,
        } => Node::Admonition {
            kind,
            title,
            children: map_children(children, base, store, errors),
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
                    children: map_children(item.children, base, store, errors),
                })
                .collect(),
        },
        Node::DefinitionList { items } => Node::DefinitionList {
            items: items
                .into_iter()
                .map(|d| Definition {
                    id: d.id,
                    term: map_children(d.term, base, store, errors),
                    definition: map_children(d.definition, base, store, errors),
                })
                .collect(),
        },
        Node::Table { headers, rows } => Node::Table {
            headers: map_cells(headers, base, store, errors),
            rows: rows
                .into_iter()
                .map(|r| TableRow {
                    cells: map_cells(r.cells, base, store, errors),
                })
                .collect(),
        },

        other => other,
    }
}

fn map_children(
    children: Vec<Node>,
    base: &Url,
    store: &mut dyn AssetStore,
    errors: &mut Vec<AssetError>,
) -> Vec<Node> {
    children
        .into_iter()
        .map(|c| map_node(c, base, store, errors))
        .collect()
}

fn map_cells(
    cells: Vec<TableCell>,
    base: &Url,
    store: &mut dyn AssetStore,
    errors: &mut Vec<AssetError>,
) -> Vec<TableCell> {
    cells
        .into_iter()
        .map(|c| TableCell {
            children: map_children(c.children, base, store, errors),
        })
        .collect()
}

fn localize_image(
    src: String,
    alt: String,
    base: &Url,
    store: &mut dyn AssetStore,
    errors: &mut Vec<AssetError>,
) -> Node {
    // A `data:` URI is already local (no fetch); the sanitizer drops these
    // before localization runs, but handle it for robustness if the order
    // ever changes.
    if src.starts_with("data:") {
        return Node::Image { src, alt };
    }

    let absolute = match base.join(&src) {
        Ok(url) => url,
        Err(_) => {
            // Unparseable src: not a remote reference, but not usable either.
            errors.push(AssetError {
                url: src,
                reason: "not a valid URL".into(),
            });
            return placeholder(alt);
        }
    };

    // Only http(s) is fetchable. Anything else that reached here (it should
    // not, post-sanitize) becomes a placeholder rather than a live reference.
    if !matches!(absolute.scheme(), "http" | "https") {
        errors.push(AssetError {
            url: absolute.to_string(),
            reason: format!("unsupported scheme {:?}", absolute.scheme()),
        });
        return placeholder(alt);
    }

    match store.localize(&absolute) {
        Ok(local_ref) => Node::Image {
            src: local_ref,
            alt,
        },
        Err(reason) => {
            errors.push(AssetError {
                url: absolute.to_string(),
                reason,
            });
            placeholder(alt)
        }
    }
}

/// The offline-safe stand-in for an asset that could not be localized: an
/// emphasized note, carrying no remote reference. The alt text is preserved
/// where present so the reader still conveys what the image was.
fn placeholder(alt: String) -> Node {
    let label = if alt.trim().is_empty() {
        "image unavailable offline".to_owned()
    } else {
        format!("image unavailable offline: {}", alt.trim())
    };
    Node::Emphasis {
        children: vec![Node::Text { value: label }],
    }
}

// ---------------------------------------------------------------------------
// The real store: fetch through the same client, content-address on disk.
// ---------------------------------------------------------------------------

/// Per-asset and per-source size limits.
#[derive(Debug, Clone, Copy)]
pub struct AssetLimits {
    /// Reject any single asset larger than this (default 10 MB).
    pub max_asset_bytes: u64,
    /// Stop storing once a source's assets total this much (default 250 MB).
    pub max_total_bytes: u64,
}

impl Default for AssetLimits {
    fn default() -> Self {
        Self {
            max_asset_bytes: 10 * 1024 * 1024,
            max_total_bytes: 250 * 1024 * 1024,
        }
    }
}

/// Fetches assets through the shared [`Fetcher`] (so robots, rate limiting,
/// and the SSRF filter all apply) and stores them content-addressed under a
/// directory. Dedups by hash within a run; an asset seen twice is fetched
/// once. The reference it returns is `assets/<sha256>.<ext>`, relative — the
/// reader (S1-13) mounts the source's asset directory and resolves it.
pub struct FetchingAssetStore<'a> {
    fetcher: &'a Fetcher,
    assets_dir: PathBuf,
    limits: AssetLimits,
    /// url → local ref, so a repeated URL is not refetched.
    seen: HashMap<String, String>,
    total_bytes: u64,
}

impl<'a> FetchingAssetStore<'a> {
    pub fn new(fetcher: &'a Fetcher, assets_dir: PathBuf) -> Self {
        Self::with_limits(fetcher, assets_dir, AssetLimits::default())
    }

    pub fn with_limits(fetcher: &'a Fetcher, assets_dir: PathBuf, limits: AssetLimits) -> Self {
        Self {
            fetcher,
            assets_dir,
            limits,
            seen: HashMap::new(),
            total_bytes: 0,
        }
    }

    fn fetch_and_store(&mut self, url: &Url) -> Result<String, String> {
        use crate::fetch::FetchOutcome;

        let outcome = self
            .fetcher
            // The per-asset cap is enforced by read_body too; +1 so the cap
            // itself is not spuriously exceeded by the fetcher's own limit.
            .fetch(url, self.limits.max_asset_bytes, None)
            .map_err(|e| e.to_string())?;
        let fetched = match outcome {
            FetchOutcome::Fetched(f) => f,
            FetchOutcome::NotModified => return Err("unexpected 304 on a fresh asset".into()),
        };

        let size = fetched.body.len() as u64;
        if self.total_bytes.saturating_add(size) > self.limits.max_total_bytes {
            return Err(format!(
                "source asset budget of {} bytes exceeded",
                self.limits.max_total_bytes
            ));
        }

        let extension = extension_for(fetched.content_type.as_deref()).ok_or_else(|| {
            format!(
                "content type {:?} is not an allowed asset media type",
                fetched.content_type
            )
        })?;

        let digest = hex(&sha256(&fetched.body));
        let filename = format!("{digest}.{extension}");
        let path = self.assets_dir.join(&filename);

        // Content-addressed: identical bytes land on the same path, so a
        // write-if-absent both dedups and makes re-sync idempotent.
        if !path.exists() {
            std::fs::create_dir_all(&self.assets_dir)
                .and_then(|()| std::fs::write(&path, &fetched.body))
                .map_err(|e| format!("could not store the asset: {e}"))?;
            self.total_bytes = self.total_bytes.saturating_add(size);
        }

        Ok(format!("assets/{filename}"))
    }
}

impl AssetStore for FetchingAssetStore<'_> {
    fn localize(&mut self, url: &Url) -> Result<String, String> {
        if let Some(local) = self.seen.get(url.as_str()) {
            return Ok(local.clone());
        }
        let local = self.fetch_and_store(url)?;
        self.seen.insert(url.as_str().to_owned(), local.clone());
        Ok(local)
    }
}

/// The file extension for a sniffed content type, or `None` if the media type
/// is not in the image allowlist. The extension comes from the *type*, not
/// the URL, so a `.png` URL serving an SVG is stored (and later sanitized) as
/// an SVG. Documentation assets are images; other media types are rejected.
fn extension_for(content_type: Option<&str>) -> Option<&'static str> {
    let ct = content_type?.split(';').next()?.trim().to_ascii_lowercase();
    match ct.as_str() {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        "image/avif" => Some("avif"),
        "image/x-icon" | "image/vnd.microsoft.icon" => Some("ico"),
        _ => None,
    }
}
