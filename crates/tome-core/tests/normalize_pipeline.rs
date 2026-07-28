//! Normalization pipeline unit behaviour (S1-8). The golden corpus
//! (`corpus/normalization`) covers whole real pages; these pin the
//! individual transforms.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use tome_core::model::Node;
use tome_core::normalize::normalize;
use tome_core::parse::parse_page;
use url::Url;

fn base() -> Url {
    "https://docs.example.test/guide/index.html"
        .parse()
        .unwrap()
}

fn norm(html: &str) -> tome_core::normalize::NormalizedPage {
    let parsed = parse_page(html, &base(), None);
    normalize(parsed.body, &base())
}

fn children(body: &Node) -> &[Node] {
    match body {
        Node::Document { children } => children,
        other => panic!("expected Document, got {other:?}"),
    }
}

#[test]
fn relative_urls_are_resolved_to_absolute() {
    let page = norm(
        r#"<main><p><a href="../api/os.html">os</a> and <img src="img/x.png" alt="x"></p></main>"#,
    );
    let json = serde_json::to_string(&page.body).unwrap();
    assert!(
        json.contains("https://docs.example.test/api/os.html"),
        "{json}"
    );
    assert!(
        json.contains("https://docs.example.test/guide/img/x.png"),
        "{json}"
    );
}

#[test]
fn absolute_and_external_urls_are_left_alone() {
    let page = norm(r#"<main><p><a href="https://other.test/x">ext</a></p></main>"#);
    let json = serde_json::to_string(&page.body).unwrap();
    assert!(json.contains("https://other.test/x"));
}

#[test]
fn headings_are_shifted_so_the_top_becomes_h1() {
    // A page whose shallowest heading is h2 (common in rustdoc / hand-written
    // pages): everything shifts up by one so the title is h1 and its
    // sub-sections are h2, hierarchy preserved.
    let page = norm("<main><h2>Title</h2><p>x</p><h3>Section</h3><h4>Sub</h4></main>");
    let levels: Vec<u8> = collect_heading_levels(&page.body);
    assert_eq!(levels, vec![1, 2, 3], "h2/h3/h4 -> h1/h2/h3");
    assert_eq!(page.title.as_deref(), Some("Title"));
}

#[test]
fn headings_already_leading_with_h1_are_untouched() {
    let page = norm("<main><h1>Title</h1><h2>Section</h2><h3>Sub</h3></main>");
    assert_eq!(collect_heading_levels(&page.body), vec![1, 2, 3]);
}

#[test]
fn heading_shift_never_produces_level_zero_or_overflows() {
    // Deeply nested starting point; the clamp to 1..=6 must hold.
    let page = norm("<main><h6>Deep</h6></main>");
    assert_eq!(collect_heading_levels(&page.body), vec![1]);
}

#[test]
fn code_languages_are_canonicalized() {
    for (input, expected) in [
        ("rs", "rust"),
        ("py3", "python"),
        ("js", "javascript"),
        ("Console", "bash"),
        ("YML", "yaml"),
    ] {
        let page = norm(&format!(
            "<main><pre><code class=\"language-{input}\">x</code></pre></main>"
        ));
        let lang = collect_first_code_language(&page.body);
        assert_eq!(lang.as_deref(), Some(expected), "{input}");
    }
}

#[test]
fn unknown_code_language_is_lowercased_and_kept() {
    let page = norm("<main><pre><code class=\"language-Brainfuck\">+</code></pre></main>");
    assert_eq!(
        collect_first_code_language(&page.body).as_deref(),
        Some("brainfuck")
    );
}

#[test]
fn empty_paragraphs_and_whitespace_text_are_pruned() {
    let page = norm("<main><p>real</p><p>   </p><p></p></main>");
    let paragraphs = children(&page.body)
        .iter()
        .filter(|n| matches!(n, Node::Paragraph { .. }))
        .count();
    assert_eq!(paragraphs, 1, "only the non-empty paragraph survives");
}

#[test]
fn metadata_is_extracted() {
    let page = norm(
        "<main><h1>The Title</h1><p>The first paragraph describes the page in some detail.</p></main>",
    );
    assert_eq!(page.title.as_deref(), Some("The Title"));
    assert!(page
        .description
        .as_deref()
        .unwrap()
        .starts_with("The first paragraph"));
}

#[test]
fn description_is_truncated_on_a_word_boundary() {
    let long = "word ".repeat(100);
    let page = norm(&format!("<main><h1>T</h1><p>{long}</p></main>"));
    let desc = page.description.unwrap();
    assert!(
        desc.chars().count() <= 201,
        "should be ~200 chars, was {}",
        desc.chars().count()
    );
    assert!(desc.ends_with('…'));
    assert!(!desc.contains("wor…"), "must break on a word, not mid-word");
}

#[test]
fn a_page_with_no_heading_has_no_title() {
    let page = norm("<main><p>just prose</p></main>");
    assert_eq!(page.title, None);
    assert!(page.description.is_some());
}

// ---- helpers ---------------------------------------------------------------

fn collect_heading_levels(node: &Node) -> Vec<u8> {
    let mut out = Vec::new();
    fn walk(node: &Node, out: &mut Vec<u8>) {
        if let Node::Heading { level, .. } = node {
            out.push(*level);
        }
        for child in direct_children(node) {
            walk(child, out);
        }
    }
    walk(node, &mut out);
    out
}

fn collect_first_code_language(node: &Node) -> Option<String> {
    if let Node::CodeBlock { language, .. } = node {
        return language.clone();
    }
    direct_children(node)
        .iter()
        .find_map(|c| collect_first_code_language(c))
}

fn direct_children(node: &Node) -> Vec<&Node> {
    match node {
        Node::Document { children }
        | Node::Heading { children, .. }
        | Node::Paragraph { children }
        | Node::Blockquote { children }
        | Node::Admonition { children, .. } => children.iter().collect(),
        _ => Vec::new(),
    }
}
