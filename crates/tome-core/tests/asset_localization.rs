//! Asset localization (S1-10): the AST transform with a fake store, and the
//! real store against the fixture server — including the offline assertion
//! that is Stage 1's exit gate.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::HashMap;
use std::time::Duration;

use tome_core::assets::{localize_assets, AssetStore, FetchingAssetStore};
use tome_core::config::FetchConfig;
use tome_core::fetch::Fetcher;
use tome_core::model::Node;
use tome_core::parse::parse_page;
use url::Url;

fn base() -> Url {
    "https://docs.example.test/guide/index.html"
        .parse()
        .unwrap()
}

/// A store that hands back a deterministic local ref and records every URL it
/// was asked to localize. `fail_contains` makes any URL containing that
/// substring fail, to exercise the placeholder path.
struct FakeStore {
    calls: Vec<String>,
    refs: HashMap<String, String>,
    fail_contains: Option<String>,
    next: usize,
}

impl FakeStore {
    fn new() -> Self {
        Self {
            calls: Vec::new(),
            refs: HashMap::new(),
            fail_contains: None,
            next: 0,
        }
    }
    fn failing(substr: &str) -> Self {
        let mut s = Self::new();
        s.fail_contains = Some(substr.to_owned());
        s
    }
}

impl AssetStore for FakeStore {
    fn localize(&mut self, url: &Url) -> Result<String, String> {
        self.calls.push(url.to_string());
        if let Some(sub) = &self.fail_contains {
            if url.as_str().contains(sub.as_str()) {
                return Err("scripted failure".into());
            }
        }
        if let Some(existing) = self.refs.get(url.as_str()) {
            return Ok(existing.clone());
        }
        self.next += 1;
        let local = format!("assets/fake{}.png", self.next);
        self.refs.insert(url.as_str().to_owned(), local.clone());
        Ok(local)
    }
}

fn srcs(node: &Node, out: &mut Vec<String>) {
    if let Node::Image { src, .. } = node {
        out.push(src.clone());
    }
    for child in children(node) {
        srcs(child, out);
    }
}

fn children(node: &Node) -> Vec<&Node> {
    match node {
        Node::Document { children }
        | Node::Heading { children, .. }
        | Node::Paragraph { children }
        | Node::Emphasis { children }
        | Node::Strong { children }
        | Node::Blockquote { children }
        | Node::Admonition { children, .. }
        | Node::Link { children, .. } => children.iter().collect(),
        Node::List { items, .. } => items.iter().flat_map(|i| i.children.iter()).collect(),
        _ => Vec::new(),
    }
}

fn parse(html: &str) -> Node {
    parse_page(html, &base(), None).body
}

// ---- the AST transform, with a fake store ----------------------------------

#[test]
fn image_srcs_are_rewritten_to_local_refs() {
    let body = parse(r#"<main><p><img src="../img/logo.png" alt="Logo"></p></main>"#);
    let mut store = FakeStore::new();
    let out = localize_assets(body, &base(), &mut store);

    // The store saw the absolute-resolved URL.
    assert_eq!(store.calls, ["https://docs.example.test/img/logo.png"]);
    // The AST now points at a local ref, no http.
    let mut found = Vec::new();
    srcs(&out.body, &mut found);
    assert_eq!(found, ["assets/fake1.png"]);
    assert!(out.errors.is_empty());
}

#[test]
fn identical_urls_are_localized_once() {
    let body = parse(
        r#"<main><p><img src="logo.png" alt="a"></p><p><img src="logo.png" alt="b"></p></main>"#,
    );
    let mut store = FakeStore::new();
    let out = localize_assets(body, &base(), &mut store);

    // Two images, one URL — the transform calls the store for each image, but
    // a real store dedups; here both resolve to the same ref.
    let mut found = Vec::new();
    srcs(&out.body, &mut found);
    assert_eq!(found, ["assets/fake1.png", "assets/fake1.png"]);
}

#[test]
fn a_failed_asset_becomes_an_offline_safe_placeholder_not_a_remote_ref() {
    let body = parse(r#"<main><p><img src="https://cdn.test/x.png" alt="Diagram"></p></main>"#);
    let mut store = FakeStore::failing("cdn.test");
    let out = localize_assets(body, &base(), &mut store);

    // No image src survives at all — the failed image became a note.
    let mut found = Vec::new();
    srcs(&out.body, &mut found);
    assert!(
        found.is_empty(),
        "a failed asset must not remain an <img>: {found:?}"
    );

    // The placeholder keeps the alt text and records the failure.
    assert!(out.body.text_content().contains("Diagram"));
    assert_eq!(out.errors.len(), 1);
    assert!(out.errors[0].url.contains("cdn.test"));

    // The offline invariant: nothing http survived.
    let json = serde_json::to_string(&out.body).unwrap();
    assert!(
        !json.contains("http"),
        "a remote reference survived: {json}"
    );
}

#[test]
fn data_uris_pass_through_without_a_fetch() {
    // (The sanitizer drops data: images before this stage, but the transform
    // must not fetch one if it sees it.)
    let body = Node::Document {
        children: vec![Node::Image {
            src: "data:image/png;base64,iVBORw0KG".into(),
            alt: "inline".into(),
        }],
    };
    let mut store = FakeStore::new();
    let out = localize_assets(body, &base(), &mut store);
    assert!(store.calls.is_empty(), "data: URIs must not be fetched");
    let mut found = Vec::new();
    srcs(&out.body, &mut found);
    assert_eq!(found, ["data:image/png;base64,iVBORw0KG"]);
}

// ---- the real store, against the fixture server ----------------------------

fn fixture_fetcher() -> Fetcher {
    let config = FetchConfig {
        rate_limit_rps: 1000.0,
        allow_insecure: true, // loopback fixture
        timeout: Duration::from_millis(800),
        ..FetchConfig::default()
    };
    Fetcher::with_backoff_base(config, Duration::from_millis(5))
}

#[test]
fn the_fetching_store_content_addresses_assets_and_renders_offline() {
    use tome_testkit::FixtureServer;

    let server = FixtureServer::start("sphinx-example").unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let assets_dir = tmp.path().join("assets");

    // A page that references the fixture's real SVG logo.
    let logo = server.url_for("/_static/logo.svg");
    let page_url: Url = server.url_for("/index.html").parse().unwrap();
    let html = format!(r#"<main><p><img src="{logo}" alt="Logo"></p></main>"#);
    let body = parse_page(&html, &page_url, None).body;

    let fetcher = fixture_fetcher();
    let out = {
        let mut store = FetchingAssetStore::new(&fetcher, assets_dir.clone());
        localize_assets(body, &page_url, &mut store)
    };

    assert!(
        out.errors.is_empty(),
        "unexpected asset errors: {:?}",
        out.errors
    );

    // The image now points at a content-addressed local file that exists.
    let mut found = Vec::new();
    srcs(&out.body, &mut found);
    assert_eq!(found.len(), 1);
    let local_ref = &found[0];
    assert!(
        local_ref.starts_with("assets/") && local_ref.ends_with(".svg"),
        "{local_ref}"
    );
    let filename = local_ref.strip_prefix("assets/").unwrap();
    assert!(
        assets_dir.join(filename).exists(),
        "asset file was not written"
    );

    // The offline assertion — Stage 1's exit gate. Shut the server down so
    // any surviving remote reference would fail loudly, and confirm the
    // rendered AST has none.
    server.shutdown();
    let json = serde_json::to_string(&out.body).unwrap();
    assert!(
        !json.contains("http") && !json.contains(&server.addr().to_string()),
        "a remote reference survived localization: {json}"
    );
}

#[test]
fn assets_are_deduplicated_across_pages() {
    use tome_testkit::FixtureServer;

    let server = FixtureServer::start("sphinx-example").unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let fetcher = fixture_fetcher();
    let mut store = FetchingAssetStore::new(&fetcher, tmp.path().join("assets"));

    let logo = server.url_for("/_static/logo.svg");
    let page_url: Url = server.url_for("/index.html").parse().unwrap();

    // Two pages, same logo.
    for _ in 0..2 {
        let html = format!(r#"<main><img src="{logo}" alt="l"></main>"#);
        let body = parse_page(&html, &page_url, None).body;
        localize_assets(body, &page_url, &mut store);
    }

    // The logo was fetched from the server exactly once.
    assert_eq!(
        server.requests_for("/_static/logo.svg").len(),
        1,
        "the store must fetch a repeated asset only once"
    );
}
