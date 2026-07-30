//! The MCP tools (P4-015/P4-016): what an agent can do with the library.
//!
//! Every tool returns **text**, not JSON — the consumer is a model reading
//! prose, and a page of JSON-escaped markdown is strictly worse for it than
//! the markdown. Errors follow the same rule: `Err` carries a sentence that
//! names the remedy (`tome pull`, the right tool to call instead), because
//! the model can act on "no source named X; call tome_list_sources" and
//! cannot act on a bare error code.
//!
//! Two tools from P4-013's sketch are deliberately absent:
//!
//! * **`tome_bookmark`** — there is no bookmark model until Phase 3, and
//!   P4-013 makes write-capable tools opt-in besides: the docs Tome ingests
//!   are untrusted text that agents read, and a prompt injection that can
//!   silently mutate the user's library is a real attack path.
//! * A **`language` filter on `tome_lookup_symbol`** — symbols are extracted
//!   from headings and carry no language. Accepting a parameter that does
//!   nothing would be worse than not having it; `scope` (a source id) is the
//!   filter that exists.

use serde_json::{json, Value};
use tome_core::db::Database;
use tome_core::model::{Node, PagePath, SourceId};
use tome_core::store::PageStore;

use crate::mcp::{McpState, McpTool};

/// The byte budget for one `tome_get_page` result (S3-4).
///
/// SPIKE-008 measured why this exists: an oversized tool result does not
/// break the transport — the client diverts it to a file and the model
/// receives a filename instead of the page, which defeats the tool's whole
/// purpose. 48 KiB is ~12k tokens, comfortably inside Claude Code's default
/// 25k-token cap with room for the rest of the turn. Truncation happens at a
/// block boundary and the notice lists the page's sections, so the model's
/// next move — `section` — is in the text it just read.
const PAGE_BYTE_BUDGET: usize = 48 * 1024;

/// Every tool the server registers, in the order `tools/list` shows them.
pub(crate) fn all() -> Vec<Box<dyn McpTool>> {
    vec![
        Box::new(Search),
        Box::new(GetPage),
        Box::new(ListSources),
        Box::new(GetToc),
        Box::new(LookupSymbol),
    ]
}

// ---------------------------------------------------------------------------
// tome_search
// ---------------------------------------------------------------------------

struct Search;

impl McpTool for Search {
    fn name(&self) -> &'static str {
        "tome_search"
    }
    fn description(&self) -> &'static str {
        "Search the local documentation library. Returns matching pages as \
         `source_id: title (page_path)` lines. Prefix a term with @ to search \
         declared symbols only. Use tome_get_page to read a result."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search terms" },
                "scope": { "type": "string", "description": "Limit to one source id" },
                "limit": { "type": "integer", "description": "Max results (default 10)" },
            },
            "required": ["query"],
        })
    }
    fn call(&self, state: &mut McpState, arguments: &Value) -> Result<String, String> {
        let query = require_str(arguments, "query")?;
        let scope = arguments.get("scope").and_then(Value::as_str);
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .map_or(10, |l| l.clamp(1, 100) as usize);

        let engine = open_engine(state)?;
        // Over-fetch when scoping: filtering happens after ranking, same
        // honest stopgap as `tome search` (P2-016 owns scoping in-query).
        let fetch = if scope.is_some() {
            limit.saturating_mul(10).min(1000)
        } else {
            limit
        };
        let hits: Vec<_> = engine
            .search(query, fetch)
            .map_err(|e| format!("search failed: {e}"))?
            .into_iter()
            .filter(|hit| scope.is_none_or(|s| hit.source.as_str() == s))
            .take(limit)
            .collect();
        let suggestions = engine.suggest(query).unwrap_or_default();

        let mut out = String::new();
        // What the search silently corrected, said first — an answer to a
        // different question than the one asked must announce itself.
        for s in &suggestions {
            out.push_str(&format!("(searched \"{}\" for \"{}\")\n", s.meant, s.typed));
        }
        if hits.is_empty() {
            out.push_str("No results.");
            if scope.is_some() {
                out.push_str(" The scope may not match any source id — call tome_list_sources.");
            }
            return Ok(out);
        }
        for hit in &hits {
            match hit.symbol_kind {
                Some(kind) => out.push_str(&format!(
                    "{}: {} [{}] ({})\n",
                    hit.source.as_str(),
                    hit.title,
                    kind.as_str(),
                    hit.path
                )),
                None => out.push_str(&format!(
                    "{}: {} ({})\n",
                    hit.source.as_str(),
                    hit.title,
                    hit.path
                )),
            }
        }
        Ok(out.trim_end().to_owned())
    }
}

// ---------------------------------------------------------------------------
// tome_get_page
// ---------------------------------------------------------------------------

struct GetPage;

impl McpTool for GetPage {
    fn name(&self) -> &'static str {
        "tome_get_page"
    }
    fn description(&self) -> &'static str {
        "Read a documentation page from the local library as markdown. \
         Takes the source_id and page_path that tome_search returns. Long \
         pages are truncated at a block boundary; pass `section` (a heading \
         anchor id, shown as {#id}) to read one section instead."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source_id": { "type": "string", "description": "Source id, e.g. rust-std" },
                "page_path": { "type": "string", "description": "Page path within the source" },
                "section": {
                    "type": "string",
                    "description": "Heading anchor id ({#id} in page output) to read only that section",
                },
            },
            "required": ["source_id", "page_path"],
        })
    }
    fn call(&self, state: &mut McpState, arguments: &Value) -> Result<String, String> {
        let source = source_id(require_str(arguments, "source_id")?)?;
        let path = PagePath::new(require_str(arguments, "page_path")?)
            .map_err(|e| format!("page_path is not valid: {e}"))?;
        let section = arguments.get("section").and_then(Value::as_str);

        let store = PageStore::new(&state.paths, &source);
        let page = store
            .read(&path)
            .map_err(|e| format!("could not read the page: {e}"))?
            .ok_or_else(|| {
                format!(
                    "no page {path} in source `{}` — tome_search finds pages that exist",
                    source.as_str()
                )
            })?;

        let body = match section {
            Some(id) => select_section(&page.body, id).ok_or_else(|| {
                let sections = section_list(&page.body);
                if sections.is_empty() {
                    format!("this page has no section {id:?} — it has no section anchors at all; call without `section`")
                } else {
                    format!(
                        "no section {id:?} on this page. Sections:\n{sections}"
                    )
                }
            })?,
            None => page.body.clone(),
        };

        let mut out = String::new();
        render_markdown(&body, &mut out, 0);
        Ok(truncate_at_block(out.trim(), PAGE_BYTE_BUDGET, &page.body))
    }
}

/// A heading id can live in two places, because generators disagree: on the
/// heading itself, or — Sphinx's way — on a sectioning wrapper that
/// normalization reduces to a standalone [`Node::Anchor`] block immediately
/// before the heading. A "section" is either shape; both resolve to the
/// heading that carries the content.
///
/// Selection works on the document's flat block sequence. Normalization
/// flattens sectioning containers, so headings are blocks in a flat list; a
/// heading nested inside an admonition is quotation, not structure, and is
/// deliberately not addressable.
fn heading_for_anchor(children: &[Node], index: usize) -> Option<(usize, u8)> {
    // Skip any run of consecutive anchors (Sphinx can emit several targets
    // for one heading), then require a heading.
    let mut i = index;
    while matches!(children.get(i), Some(Node::Anchor { .. })) {
        i += 1;
    }
    match children.get(i) {
        Some(Node::Heading { level, .. }) => Some((i, *level)),
        _ => None,
    }
}

/// The sections of a page, as `##… title {#id}` lines. This is what the
/// truncation notice and the unknown-section error show, so the model's next
/// call needs no guessing.
fn section_list(body: &Node) -> String {
    let Node::Document { children } = body else {
        return String::new();
    };
    let mut out = String::new();
    let mut push = |level: u8, title: &str, id: &str| {
        out.push_str(&format!(
            "{} {} {{#{id}}}\n",
            "#".repeat(usize::from(level).clamp(1, 6)),
            title.trim()
        ));
    };
    for (index, node) in children.iter().enumerate() {
        match node {
            Node::Heading {
                level,
                id: Some(id),
                ..
            } => push(*level, &node.text_content(), id),
            Node::Anchor { id } => {
                if let Some((at, level)) = heading_for_anchor(children, index) {
                    push(level, &children[at].text_content(), id);
                }
            }
            _ => {}
        }
    }
    out.trim_end().to_owned()
}

/// The subtree a `section` argument names: from the section's heading (and
/// its anchors) up to — not including — the next heading of the same or a
/// higher level.
fn select_section(body: &Node, section: &str) -> Option<Node> {
    let Node::Document { children } = body else {
        return None;
    };
    // `start` is where the slice begins (the anchor, when the id lives on
    // one, so the {#id} marker survives into the output); `heading_at` is
    // where the level comes from.
    let (start, heading_at) = children
        .iter()
        .enumerate()
        .find_map(|(index, node)| match node {
            Node::Heading { id: Some(id), .. } if id == section => Some((index, Some(index))),
            Node::Anchor { id } if id == section => {
                Some((index, heading_for_anchor(children, index).map(|(at, _)| at)))
            }
            _ => None,
        })?;
    let heading_at = heading_at?;
    let Node::Heading { level, .. } = &children[heading_at] else {
        return None;
    };
    let end = children[heading_at + 1..]
        .iter()
        .position(|node| matches!(node, Node::Heading { level: l, .. } if l <= level))
        .map_or(children.len(), |offset| heading_at + 1 + offset);
    Some(Node::Document {
        children: children[start..end].to_vec(),
    })
}

/// Cut rendered markdown at the last block boundary inside `budget`, and say
/// so — with the page's sections, because "call again with `section`" is
/// only actionable if the ids are in front of the model.
///
/// A cut mid-block is never acceptable: half a code fence renders the rest
/// of the conversation as code, and half a sentence reads as complete.
fn truncate_at_block(text: &str, budget: usize, body: &Node) -> String {
    if text.len() <= budget {
        return text.to_owned();
    }

    // `text[..budget]` panics when the budget lands inside a multi-byte
    // character, and documentation is full of em dashes and curly quotes.
    // This was written once in `fetch::robots`, caught there, and written
    // again here; `tome_core::text` exists so there is one implementation to
    // reach for. Found by S4-1.
    let window = tome_core::text::truncate_at_char_boundary(text, budget);

    // Prefer a paragraph break, then a line break, then the character
    // boundary itself. The old code used `.unwrap_or(0)` for the first, so a
    // page with no blank line in its first 48 KiB — one long table, one long
    // code block — returned the truncation notice and **no content at all**,
    // reporting "showing 0 of N KiB" as though that were a result.
    let cut = window
        .rfind("\n\n")
        .or_else(|| window.rfind('\n'))
        .unwrap_or(window.len());
    let kept = &window[..cut];

    // A cut inside a code fence turns the rest of the conversation into code.
    // Counting is enough: fences nest no deeper than one.
    let closing = if kept.matches("```").count() % 2 == 1 {
        "\n```"
    } else {
        ""
    };
    let sections = section_list(body);
    let guidance = if sections.is_empty() {
        "This page has no section anchors; the rest is not reachable through this tool.".to_owned()
    } else {
        format!("Call tome_get_page again with `section` set to one of:\n{sections}")
    };
    format!(
        "{kept}{closing}\n\n[truncated: showing {} of {} KiB. {guidance}]",
        cut / 1024,
        text.len() / 1024,
    )
}

// ---------------------------------------------------------------------------
// tome_list_sources
// ---------------------------------------------------------------------------

struct ListSources;

impl McpTool for ListSources {
    fn name(&self) -> &'static str {
        "tome_list_sources"
    }
    fn description(&self) -> &'static str {
        "List the documentation sources in the local library, with page \
         counts and categories."
    }
    fn input_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    fn call(&self, state: &mut McpState, _arguments: &Value) -> Result<String, String> {
        let configs =
            crate::source_configs(&state.paths).map_err(|e| format!("could not list: {e}"))?;
        if configs.is_empty() {
            return Ok("The library is empty — no documentation sources are configured.".into());
        }
        let db = state
            .paths
            .database_file()
            .exists()
            .then(|| Database::open(&state.paths))
            .transpose()
            .map_err(|e| format!("could not open the library database: {e}"))?;
        let pulled = match &db {
            Some(db) => db
                .list_sources()
                .map_err(|e| format!("could not list sources: {e}"))?,
            None => Vec::new(),
        };

        let mut out = String::new();
        for (id, _) in &configs {
            match pulled.iter().find(|s| s.id == *id) {
                Some(source) => {
                    let pages = db
                        .as_ref()
                        .and_then(|db| db.page_count(id).ok())
                        .unwrap_or(source.page_count);
                    out.push_str(&format!(
                        "{}: {} — {} pages, category {}\n",
                        id.as_str(),
                        source.name,
                        pages,
                        source.category
                    ));
                }
                None => out.push_str(&format!(
                    "{}: configured but never pulled — no content yet\n",
                    id.as_str()
                )),
            }
        }
        Ok(out.trim_end().to_owned())
    }
}

// ---------------------------------------------------------------------------
// tome_get_toc
// ---------------------------------------------------------------------------

struct GetToc;

impl McpTool for GetToc {
    fn name(&self) -> &'static str {
        "tome_get_toc"
    }
    fn description(&self) -> &'static str {
        "The table of contents for one source: every page in navigation \
         order, as `title (page_path)` lines."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source_id": { "type": "string", "description": "Source id, e.g. rust-std" },
            },
            "required": ["source_id"],
        })
    }
    fn call(&self, state: &mut McpState, arguments: &Value) -> Result<String, String> {
        let source = source_id(require_str(arguments, "source_id")?)?;
        if !state.paths.database_file().exists() {
            return Err("nothing has been pulled yet — the library is empty".into());
        }
        let db = Database::open(&state.paths)
            .map_err(|e| format!("could not open the library database: {e}"))?;
        // `list_pages` returns navigation order (the crawl's discovery
        // order), which is the order a site's own contents list advertises —
        // not alphabetical, which opened the Cargo Book on its changelog.
        let pages = db
            .list_pages(&source)
            .map_err(|e| format!("could not list pages: {e}"))?;
        if pages.is_empty() {
            return Err(format!(
                "source `{}` has no pages — it may not exist (tome_list_sources) or has \
                 never been pulled",
                source.as_str()
            ));
        }
        let mut out = String::new();
        for page in &pages {
            out.push_str(&format!("{} ({})\n", page.title, page.path));
        }
        Ok(out.trim_end().to_owned())
    }
}

// ---------------------------------------------------------------------------
// tome_lookup_symbol
// ---------------------------------------------------------------------------

struct LookupSymbol;

impl McpTool for LookupSymbol {
    fn name(&self) -> &'static str {
        "tome_lookup_symbol"
    }
    fn description(&self) -> &'static str {
        "Find the pages that DECLARE a symbol (function, type, module) — not \
         every page that mentions it. Use for API lookups like `with_capacity` \
         or `Vec`."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": { "type": "string", "description": "The symbol name" },
                "scope": { "type": "string", "description": "Limit to one source id" },
            },
            "required": ["symbol"],
        })
    }
    fn call(&self, state: &mut McpState, arguments: &Value) -> Result<String, String> {
        let symbol = require_str(arguments, "symbol")?;
        let scope = arguments.get("scope").and_then(Value::as_str);
        // The `@` sigil is the declared-symbols-only query (P2-015); the
        // engine's parser handles the rest, including `Vec::new` syntax.
        let query = format!("@{}", symbol.trim_start_matches('@'));

        let engine = open_engine(state)?;
        let hits: Vec<_> = engine
            .search(&query, 50)
            .map_err(|e| format!("lookup failed: {e}"))?
            .into_iter()
            .filter(|hit| scope.is_none_or(|s| hit.source.as_str() == s))
            .take(10)
            .collect();

        if hits.is_empty() {
            return Ok(format!(
                "No page declares `{symbol}`. tome_search finds pages that merely mention it."
            ));
        }
        let mut out = String::new();
        for hit in &hits {
            let kind = hit.symbol_kind.map(|k| k.as_str()).unwrap_or("declares");
            out.push_str(&format!(
                "{}: {} [{kind}] ({})\n",
                hit.source.as_str(),
                hit.title,
                hit.path
            ));
        }
        Ok(out.trim_end().to_owned())
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn require_str<'a>(arguments: &'a Value, field: &str) -> Result<&'a str, String> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("`{field}` is required and must be a non-empty string"))
}

fn source_id(text: &str) -> Result<SourceId, String> {
    SourceId::new(text).map_err(|e| format!("source_id is not valid: {e}"))
}

fn open_engine(state: &mut McpState) -> Result<&tome_core::search::SearchEngine, String> {
    if !state.paths.index_dir().exists() {
        return Err("nothing has been pulled yet — the library has no search index".into());
    }
    state
        .engine()
        .map_err(|e| format!("could not open the search index: {e}"))
}

/// Render a stored page's AST as markdown for a model to read.
///
/// This is a *text* rendering, not the reader's HTML path — no escaping
/// contract applies because nothing here is markup: a `<script>` in a text
/// node stays literal text inside a text/plain tool result. Anchors and
/// images reduce to their meaning (`{#id}`, alt text); layout nuance is
/// deliberately lost. S3-4 adds section selection and truncation on top.
fn render_markdown(node: &Node, out: &mut String, depth: usize) {
    match node {
        Node::Document { children } => {
            for child in children {
                render_markdown(child, out, depth);
            }
        }
        Node::Heading {
            level,
            id,
            children,
        } => {
            out.push('\n');
            out.push_str(&"#".repeat(usize::from(*level).clamp(1, 6)));
            out.push(' ');
            for child in children {
                render_inline(child, out);
            }
            // The anchor, kept: it is what a `section` argument (S3-4) and
            // deep links refer to.
            if let Some(id) = id {
                out.push_str(&format!(" {{#{id}}}"));
            }
            out.push_str("\n\n");
        }
        Node::Paragraph { children } => {
            for child in children {
                render_inline(child, out);
            }
            out.push_str("\n\n");
        }
        Node::CodeBlock { language, code } => {
            out.push_str("```");
            if let Some(language) = language {
                out.push_str(language);
            }
            out.push('\n');
            out.push_str(code);
            if !code.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("```\n\n");
        }
        Node::Blockquote { children } => {
            let mut inner = String::new();
            for child in children {
                render_markdown(child, &mut inner, depth);
            }
            for line in inner.trim_end().lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }
        Node::List {
            ordered,
            start,
            items,
        } => {
            let indent = "  ".repeat(depth);
            for (i, item) in items.iter().enumerate() {
                let marker = if *ordered {
                    format!("{}.", start.unwrap_or(1) as usize + i)
                } else {
                    "-".to_owned()
                };
                let mut inner = String::new();
                for child in &item.children {
                    render_markdown(child, &mut inner, depth + 1);
                }
                let text = inner.trim();
                let mut lines = text.lines();
                out.push_str(&format!(
                    "{indent}{marker} {}\n",
                    lines.next().unwrap_or_default()
                ));
                for line in lines {
                    out.push_str(&format!("{indent}  {line}\n"));
                }
            }
            out.push('\n');
        }
        Node::DefinitionList { items } => {
            for item in items {
                for child in &item.term {
                    render_inline(child, out);
                }
                out.push_str("\n: ");
                let mut inner = String::new();
                for child in &item.definition {
                    render_markdown(child, &mut inner, depth);
                }
                out.push_str(inner.trim());
                out.push_str("\n\n");
            }
        }
        Node::Table { headers, rows } => {
            // A markdown pipe table; cell text only. Good enough to read,
            // not a layout engine.
            let cell_text = |cells: &[tome_core::model::TableCell]| -> Vec<String> {
                cells
                    .iter()
                    .map(|cell| {
                        let mut text = String::new();
                        for child in &cell.children {
                            render_inline(child, &mut text);
                        }
                        text.replace('|', "\\|").trim().to_owned()
                    })
                    .collect()
            };
            let headers = cell_text(headers);
            if !headers.is_empty() {
                out.push_str(&format!("| {} |\n", headers.join(" | ")));
                out.push_str(&format!("|{}\n", " --- |".repeat(headers.len())));
            }
            for row in rows {
                let cells = cell_text(&row.cells);
                out.push_str(&format!("| {} |\n", cells.join(" | ")));
            }
            out.push('\n');
        }
        Node::Admonition {
            kind,
            title,
            children,
        } => {
            out.push_str(&format!(
                "> **{}**",
                title.clone().unwrap_or_else(|| kind.clone())
            ));
            out.push('\n');
            let mut inner = String::new();
            for child in children {
                render_markdown(child, &mut inner, depth);
            }
            for line in inner.trim_end().lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }
        Node::ThematicBreak {} => out.push_str("---\n\n"),
        // Inline content at block level — normalization should not produce
        // it, but a renderer that drops content it does not expect is how
        // pages lose text silently.
        other => {
            render_inline(other, out);
            out.push('\n');
        }
    }
}

fn render_inline(node: &Node, out: &mut String) {
    match node {
        Node::Text { value } => out.push_str(value),
        Node::InlineCode { code } => out.push_str(&format!("`{code}`")),
        Node::Emphasis { children } => {
            out.push('*');
            for child in children {
                render_inline(child, out);
            }
            out.push('*');
        }
        Node::Strong { children } => {
            out.push_str("**");
            for child in children {
                render_inline(child, out);
            }
            out.push_str("**");
        }
        Node::Link { href, children, .. } => {
            out.push('[');
            for child in children {
                render_inline(child, out);
            }
            out.push_str(&format!("]({href})"));
        }
        Node::Image { alt, .. } => {
            // The asset is a local file the model cannot see; the alt text
            // is the part that carries meaning.
            out.push_str(&format!("[image: {alt}]"));
        }
        Node::Anchor { id } => out.push_str(&format!("{{#{id}}}")),
        Node::LineBreak {} => out.push('\n'),
        // A block node in inline position: fall back to its text content
        // rather than dropping it.
        other => out.push_str(&other.text_content()),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn heading(level: u8, id: &str, text: &str) -> Node {
        Node::Heading {
            level,
            id: Some(id.into()),
            children: vec![Node::Text { value: text.into() }],
        }
    }
    fn para(text: &str) -> Node {
        Node::Paragraph {
            children: vec![Node::Text { value: text.into() }],
        }
    }
    fn document() -> Node {
        Node::Document {
            children: vec![
                heading(1, "top", "Widget"),
                para("intro"),
                heading(2, "install", "Install"),
                para("install text"),
                heading(3, "from-source", "From source"),
                para("build text"),
                heading(2, "usage", "Usage"),
                para("usage text"),
            ],
        }
    }

    #[test]
    fn a_section_runs_to_the_next_same_or_higher_heading() {
        // #install (h2) owns its h3 subsection but stops at #usage (h2) —
        // the definition of "subtree", and the case a naive "until the next
        // heading" gets wrong.
        let section = select_section(&document(), "install").expect("found");
        let mut out = String::new();
        render_markdown(&section, &mut out, 0);
        assert!(out.contains("install text"), "own content: {out}");
        assert!(out.contains("build text"), "subsection kept: {out}");
        assert!(!out.contains("usage text"), "stops at sibling: {out}");
        assert!(!out.contains("intro"), "starts at the heading: {out}");
    }

    #[test]
    fn a_sphinx_shaped_anchor_before_the_heading_is_the_section_id() {
        // What the fixture actually stores: Sphinx puts the id on a section
        // wrapper, and normalization reduces it to an Anchor block *before*
        // a heading whose own id is None. The first version of this code
        // only looked at heading ids and reported "no section anchors at
        // all" for every Sphinx page.
        let body = Node::Document {
            children: vec![
                Node::Anchor { id: "top".into() },
                Node::Heading {
                    level: 1,
                    id: None,
                    children: vec![Node::Text {
                        value: "Widget".into(),
                    }],
                },
                para("intro"),
                Node::Anchor {
                    id: "examples".into(),
                },
                Node::Heading {
                    level: 2,
                    id: None,
                    children: vec![Node::Text {
                        value: "Examples".into(),
                    }],
                },
                para("example text"),
            ],
        };
        let list = section_list(&body);
        assert!(list.contains("{#examples}"), "anchor id listed: {list}");
        assert!(list.contains("## Examples"), "heading level kept: {list}");

        let section = select_section(&body, "examples").expect("found by anchor");
        let mut out = String::new();
        render_markdown(&section, &mut out, 0);
        assert!(out.contains("example text"), "{out}");
        assert!(!out.contains("intro"), "{out}");
        assert!(
            out.contains("{#examples}"),
            "the anchor survives into the output so the id stays visible: {out}"
        );
    }

    #[test]
    fn the_last_section_runs_to_the_end() {
        let section = select_section(&document(), "usage").expect("found");
        let mut out = String::new();
        render_markdown(&section, &mut out, 0);
        assert!(out.contains("usage text"), "{out}");
    }

    #[test]
    fn an_unknown_section_is_none_and_the_list_names_the_real_ones() {
        assert!(select_section(&document(), "nope").is_none());
        let list = section_list(&document());
        for id in ["{#top}", "{#install}", "{#from-source}", "{#usage}"] {
            assert!(list.contains(id), "{id} listed: {list}");
        }
    }

    #[test]
    fn truncation_cuts_at_a_block_boundary_and_names_the_sections() {
        let body = document();
        // Blocks of 1000 'x's: the cut must land between blocks, never
        // inside one — half a code fence would swallow the conversation.
        let long: String = (0..100)
            .map(|_| "x".repeat(1000))
            .collect::<Vec<_>>()
            .join("\n\n");
        let out = truncate_at_block(&long, 10_000, &body);
        assert!(out.len() < 12_000, "budget respected: {} bytes", out.len());
        assert!(out.contains("[truncated"), "notice present");
        assert!(out.contains("{#install}"), "sections listed: {out}");
        // Every kept block is intact.
        let kept = out.split("\n\n[truncated").next().expect("kept part");
        for block in kept.split("\n\n") {
            assert_eq!(block.len(), 1000, "no block was cut in half");
        }
    }

    #[test]
    fn short_output_is_untouched() {
        let text = "short page";
        assert_eq!(truncate_at_block(text, 1000, &document()), text);
    }

    // --- truncation (S4-1: three defects, all silent) ----------------------

    #[test]
    fn a_multibyte_character_on_the_budget_boundary_does_not_panic() {
        // `text[..budget]` panics when the budget lands inside a character,
        // and documentation is full of em dashes, curly quotes and accented
        // names — so for a long enough page the odds are close to even.
        let budget = 64;
        let mut text = "x".repeat(budget - 1);
        text.push('—'); // three bytes, straddling the boundary
        text.push_str(&"y".repeat(100));
        let out = truncate_at_block(&text, budget, &Node::Document { children: vec![] });
        assert!(out.contains("truncated"), "{out}");
    }

    #[test]
    fn a_page_with_no_paragraph_break_still_returns_content() {
        // One long table, or one long code block. The old code cut at 0 and
        // returned the notice alone, reporting "showing 0 of N KiB" as though
        // that were a result.
        let text = format!("| a | b |\n{}", "| 1 | 2 |\n".repeat(200));
        let out = truncate_at_block(&text, 256, &Node::Document { children: vec![] });
        assert!(
            out.lines().next().is_some_and(|line| line.contains('|')),
            "the reply must carry page content: {out}"
        );
    }

    #[test]
    fn a_cut_inside_a_code_fence_closes_it() {
        // Otherwise the truncation notice — and everything the model says
        // afterwards — is inside a code block.
        let text = format!("```rust\n{}\n```\n", "let x = 1;\n".repeat(200));
        let out = truncate_at_block(&text, 256, &Node::Document { children: vec![] });
        assert_eq!(
            out.matches("```").count() % 2,
            0,
            "unbalanced fences in: {out}"
        );
    }
}
