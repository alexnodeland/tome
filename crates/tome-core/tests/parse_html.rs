//! The HTML → AST parser (S1-7): construct coverage, Sphinx idioms, and the
//! fixture page that stands in for docs.python.org until the golden corpus
//! (S1-8) takes over.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use tome_core::model::Node;
use tome_core::parse::parse_page;
use url::Url;

fn base() -> Url {
    "https://docs.example.test/guide/index.html"
        .parse()
        .unwrap()
}

fn parse(html: &str) -> tome_core::parse::ParsedPage {
    parse_page(html, &base(), None)
}

fn body_children(page: &tome_core::parse::ParsedPage) -> &[Node] {
    match &page.body {
        Node::Document { children } => children,
        other => panic!("root must be Document, got {other:?}"),
    }
}

// ---- content root -----------------------------------------------------------

#[test]
fn content_root_prefers_main_then_role_then_article() {
    let page =
        parse("<nav><p>chrome</p></nav><main><p>content</p></main><footer><p>legal</p></footer>");
    let children = body_children(&page);
    assert_eq!(children.len(), 1);
    assert_eq!(page.body.text_content(), "content");
}

#[test]
fn configured_selector_wins_but_falls_back_when_it_matches_nothing() {
    let html = "<main><p>whole main</p><div class='body'><p>inner</p></div></main>";
    let picked = parse_page(html, &base(), Some("div.body"));
    assert_eq!(picked.body.text_content(), "inner");

    let fallback = parse_page(html, &base(), Some("div.nonexistent"));
    assert_eq!(fallback.body.text_content(), "whole maininner");
}

#[test]
fn navigation_inside_content_is_still_dropped() {
    let page = parse("<main><nav><p>toc</p></nav><p>text</p><aside>ad</aside></main>");
    assert_eq!(page.body.text_content(), "text");
}

// ---- constructs ---------------------------------------------------------------

#[test]
fn headings_keep_ids_and_lose_permalink_chrome() {
    let page = parse(
        r##"<main><h2 id="setup">Setup<a class="headerlink" href="#setup" title="Permalink">¶</a></h2></main>"##,
    );
    let children = body_children(&page);
    let Node::Heading {
        level,
        id,
        children,
    } = &children[0]
    else {
        panic!("expected heading, got {children:?}");
    };
    assert_eq!(*level, 2);
    assert_eq!(id.as_deref(), Some("setup"));
    assert_eq!(children.len(), 1, "the ¶ permalink must be stripped");
}

#[test]
fn code_blocks_pick_up_language_hints_from_both_conventions() {
    // Markdown/MkDocs convention: class on <code>.
    let md = parse("<main><pre><code class='language-rust'>fn x() {}</code></pre></main>");
    let Node::CodeBlock { language, code } = &body_children(&md)[0] else {
        panic!("expected code block")
    };
    assert_eq!(language.as_deref(), Some("rust"));
    assert_eq!(code, "fn x() {}");

    // Sphinx convention: highlight-<lang> on the wrapper.
    let sphinx = parse(
        "<main><div class='highlight-python notranslate'><div class='highlight'><pre>import os</pre></div></div></main>",
    );
    // The wrapper divs unwrap; the pre must still find the language.
    let found = body_children(&sphinx).iter().find_map(|n| match n {
        Node::CodeBlock { language, .. } => language.as_deref(),
        _ => None,
    });
    assert_eq!(found, Some("python"));
}

#[test]
fn pre_preserves_interior_whitespace_while_prose_collapses() {
    let page = parse("<main><p>a   b\n  c</p><pre>line1\n    indented</pre></main>");
    let children = body_children(&page);
    assert!(matches!(&children[0], Node::Paragraph { children }
        if matches!(&children[0], Node::Text { value } if value == "a b c")));
    assert!(matches!(&children[1], Node::CodeBlock { code, .. }
        if code == "line1\n    indented"));
}

#[test]
fn definition_lists_survive_with_ids_terms_and_definitions() {
    // The Sphinx API-entry shape — the reason DefinitionList is first-class.
    let page = parse(
        r##"<main><dl class="py class">
            <dt id="widget.Widget"><span class="sig-name">Widget</span><a class="headerlink" href="#widget.Widget">¶</a></dt>
            <dd><p>A widget.</p></dd>
        </dl></main>"##,
    );
    let Node::DefinitionList { items } = &body_children(&page)[0] else {
        panic!("expected definition list")
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id.as_deref(), Some("widget.Widget"));
    assert_eq!(
        items[0]
            .term
            .iter()
            .map(Node::text_content)
            .collect::<String>(),
        "Widget",
        "sig text kept, permalink stripped"
    );
    assert!(items[0]
        .definition
        .iter()
        .any(|n| n.text_content() == "A widget."));
}

#[test]
fn multiple_dt_before_one_dd_and_orphan_dd_both_survive() {
    let page = parse("<main><dl><dt>a</dt><dt>b</dt><dd>both</dd><dd>orphan</dd></dl></main>");
    let Node::DefinitionList { items } = &body_children(&page)[0] else {
        panic!("expected definition list")
    };
    // dt a (no dd), dt b (dd "both"), orphan dd appended.
    assert_eq!(items.len(), 3);
    assert!(items[0].definition.is_empty());
    assert_eq!(items[1].definition[0].text_content(), "both");
    assert!(items[2].term.is_empty());
}

#[test]
fn tables_split_header_and_body_rows() {
    let page = parse(
        "<main><table><thead><tr><th>k</th><th>v</th></tr></thead>
         <tbody><tr><td>a</td><td>1</td></tr></tbody></table></main>",
    );
    let Node::Table { headers, rows } = &body_children(&page)[0] else {
        panic!("expected table")
    };
    assert_eq!(headers.len(), 2);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].cells[0].children[0].text_content(), "a");
}

#[test]
fn admonitions_keep_kind_title_and_body() {
    let page = parse(
        "<main><div class='admonition warning'>
            <p class='admonition-title'>Warning</p>
            <p>Do not.</p>
         </div></main>",
    );
    let Node::Admonition {
        kind,
        title,
        children,
    } = &body_children(&page)[0]
    else {
        panic!("expected admonition")
    };
    assert_eq!(kind, "warning");
    assert_eq!(title.as_deref(), Some("Warning"));
    assert_eq!(children[0].text_content(), "Do not.");
}

#[test]
fn unknown_wrappers_unwrap_but_keep_their_anchor() {
    let page = parse("<main><div id='section-3' class='mystery'><p>kept</p></div></main>");
    let children = body_children(&page);
    assert!(matches!(&children[0], Node::Anchor { id } if id == "section-3"));
    assert!(matches!(&children[1], Node::Paragraph { .. }));
}

#[test]
fn empty_sphinx_span_anchors_survive() {
    let page = parse("<main><span id='module-widget'></span><p>text</p></main>");
    assert!(matches!(
        &body_children(&page)[0],
        Node::Anchor { id } if id == "module-widget"
    ));
}

#[test]
fn scripts_styles_and_event_bearing_junk_never_enter_the_ast() {
    let page = parse(
        "<main><script>window.x=1</script><style>p{}</style>
         <p>real</p><iframe src='x'></iframe><form><input></form></main>",
    );
    assert_eq!(page.body.text_content(), "real");
    // Nothing in the AST can carry the script text — there is no node type
    // for it (see model/node.rs on the absent Raw variant).
    let json = serde_json::to_string(&page.body).unwrap();
    assert!(!json.contains("window.x"));
}

// ---- title and links -------------------------------------------------------------

#[test]
fn title_comes_from_title_tag_then_first_h1_then_none() {
    let titled = parse("<head><title>The   Title</title></head><body><main><p>x</p></main></body>");
    assert_eq!(titled.title.as_deref(), Some("The Title"));

    let h1 = parse("<main><h1>Heading Title</h1></main>");
    assert_eq!(h1.title.as_deref(), Some("Heading Title"));

    let none = parse("<main><p>no title anywhere</p></main>");
    assert_eq!(none.title, None);
}

#[test]
fn links_resolve_against_the_base_and_keep_as_written_in_the_ast() {
    let page = parse(
        // r## because href="#fragment" contains `"#`.
        r##"<main>
            <a href="../api/reference.html">api</a>
            <a href="/absolute.html">abs</a>
            <a href="https://other.test/x">ext</a>
            <a href="#fragment">frag</a>
            <a href="mailto:a@b.c">mail</a>
            <a href="javascript:alert(1)">js</a>
        </main>"##,
    );
    let resolved: Vec<String> = page.links.iter().map(Url::to_string).collect();
    assert!(resolved.contains(&"https://docs.example.test/api/reference.html".to_owned()));
    assert!(resolved.contains(&"https://docs.example.test/absolute.html".to_owned()));
    assert!(resolved.contains(&"https://other.test/x".to_owned()));
    // Fragment-only links resolve to the page itself (fragment dropped);
    // mailto/javascript are not crawlable and must not appear.
    assert!(!resolved.iter().any(|u| u.starts_with("mailto")));
    assert!(!resolved.iter().any(|u| u.starts_with("javascript")));

    // Inside the AST the href is exactly as written.
    let json = serde_json::to_string(&page.body).unwrap();
    assert!(json.contains("../api/reference.html"));
}

#[test]
fn duplicate_links_are_reported_once() {
    let page = parse(r#"<main><a href="a.html">1</a><a href="a.html">2</a></main>"#);
    assert_eq!(page.links.len(), 1);
}

// ---- malformed input --------------------------------------------------------------

#[test]
fn malformed_html_parses_to_something_rather_than_failing() {
    for junk in [
        "<main><p>unclosed",
        "<main><table><p>interleaved</table></p></main>",
        "<<<>>>",
        "",
        "<main><dl><dd>dd first</dd></dl></main>",
    ] {
        let page = parse(junk); // must not panic
        let _ = page.body.text_content();
    }
}

// ---- the fixture page ----------------------------------------------------------------

#[test]
fn the_sphinx_fixture_parses_into_the_expected_shape() {
    let html = std::fs::read_to_string(
        tome_testkit::server::fixtures_dir().join("sphinx-example/api/reference.html"),
    )
    .unwrap();
    let base: Url = "https://widget.readthedocs.test/api/reference.html"
        .parse()
        .unwrap();
    let page = parse_page(&html, &base, Some("div.document"));

    assert_eq!(
        page.title.as_deref(),
        Some("API reference — Widget 2.1 documentation")
    );

    // The API entry survives as a definition list with its permalink id.
    fn find_dl(nodes: &[Node]) -> Option<&Node> {
        nodes.iter().find_map(|n| match n {
            Node::DefinitionList { .. } => Some(n),
            Node::Heading { children, .. } | Node::Paragraph { children } => find_dl(children),
            _ => None,
        })
    }
    let Node::Document { children } = &page.body else {
        panic!()
    };
    let dl = find_dl(children).expect("the py class dl survives");
    let Node::DefinitionList { items } = dl else {
        panic!()
    };
    assert_eq!(items[0].id.as_deref(), Some("widget.Widget"));

    // Heading anchor for the TOC.
    assert!(children.iter().any(|n| matches!(n,
        Node::Heading { id: Some(id), .. } if id == "api-reference"
    ) || matches!(n, Node::Anchor { id } if id == "api-reference")));
}
