//! Result snippets (S2-7, spec P2-005).
//!
//! # Why this cannot use Tantivy's `SnippetGenerator`
//!
//! `SnippetGenerator` requires a **stored** field, and the schema deliberately
//! stores no body: SPIKE-003's measured 224 MB per 100 000 pages assumes it,
//! and storing the body would roughly double the index to hold a second copy
//! of bytes the [`PageStore`](crate::store::PageStore) already has.
//!
//! Reading from the store is the better place anyway. The store holds
//! structured nodes, so a snippet can respect **block boundaries** — it will
//! never run from the end of one paragraph into the beginning of the next, or
//! quote half a table cell and half a heading as though they were a sentence.
//! A generator working on the flattened field text has no way to know where
//! those boundaries were.
//!
//! # Why this returns spans rather than HTML
//!
//! The renderer's contract is that page content becomes HTML in exactly one
//! place, with every attribute quoted and every value escaped
//! ([`crate::html`]). A snippet is page content, and returning
//! `"…the <mark>environment</mark> variable…"` from here would open a second
//! path — one that has to get escaping right for ever, in a string that flows
//! straight into the app's own DOM rather than into the sandboxed reader
//! frame, where the app's origin and its IPC layer are reachable.
//!
//! So a snippet is a list of [`Span`]s, and the frontend renders each one's
//! text as a *text node*. There is no markup to escape because there is no
//! markup.

use crate::model::Node;

/// One run of snippet text, either matched or not.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Span {
    pub text: String,
    /// Whether this run is one of the query's terms, and should be marked.
    pub matched: bool,
}

/// Roughly how many characters a snippet may run to.
///
/// Not exact: the window is widened to the nearest word boundary so a snippet
/// never ends mid-word, which looks like a rendering bug rather than a
/// deliberate truncation.
const DEFAULT_LENGTH: usize = 240;

/// How many characters of lead-in to keep before the first match, so the match
/// is not flush against the left edge with no context.
const LEAD_IN: usize = 60;

/// Build a snippet for `content` highlighting `terms`.
///
/// `terms` are already-lowercased query terms — what the tokenizer produced,
/// plus any corrections applied. Returns an empty vector when the page has no
/// text at all; a page with text but no match still gets its opening block,
/// because a result with no snippet reads as a broken result.
pub fn snippet(content: &Node, terms: &[String], max_chars: usize) -> Vec<Span> {
    let max_chars = max_chars.max(32);
    let mut blocks = Vec::new();
    collect_blocks(content, &mut blocks);

    // The best block is the one matching the most *distinct* terms. Distinct
    // rather than total, or a paragraph repeating one word beats a paragraph
    // that answers the whole query.
    let best = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (distinct_matches(block, terms), index))
        .max_by_key(|(score, index)| (*score, std::cmp::Reverse(*index)));

    let Some((score, index)) = best else {
        return Vec::new();
    };
    // No block matched anything: fall back to the first, which is the page's
    // opening and the most useful thing to show.
    let block = if score == 0 {
        blocks.first()
    } else {
        blocks.get(index)
    };
    let Some(block) = block else {
        return Vec::new();
    };

    spans_for(block, terms, max_chars)
}

/// Every match range in `text`, as character indices, non-overlapping and in
/// order.
///
/// Matches are **whole tokens**, not substrings: highlighting `cat` inside
/// `concatenate` is worse than not highlighting it, because it teaches the
/// reader that the highlight means nothing.
fn match_ranges(text: &[char], terms: &[String]) -> Vec<(usize, usize)> {
    let lowered: Vec<char> = text.iter().flat_map(|c| c.to_lowercase()).collect();
    // `to_lowercase` can change the character count (`İ` becomes two chars),
    // which would slide every index afterwards. Fall back to the original when
    // that happens — a page containing such a character loses highlighting,
    // not correctness.
    let lowered = if lowered.len() == text.len() {
        lowered
    } else {
        text.to_vec()
    };

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for term in terms {
        let needle: Vec<char> = term.chars().collect();
        if needle.is_empty() || needle.len() > lowered.len() {
            continue;
        }
        for start in 0..=(lowered.len() - needle.len()) {
            let end = start + needle.len();
            if lowered[start..end] != needle[..] {
                continue;
            }
            let before_ok = start == 0 || !is_word(lowered[start - 1]);
            let after_ok = end == lowered.len() || !is_word(lowered[end]);
            if before_ok && after_ok {
                ranges.push((start, end));
            }
        }
    }

    ranges.sort_unstable();
    // Two terms can match the same span — `Vec` and `vec` after correction, or
    // a term that is also a correction of another. Overlaps would produce
    // spans that double-count characters and reassemble into text the page
    // does not contain.
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in ranges {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// How many of `terms` appear in `block` at all.
fn distinct_matches(block: &str, terms: &[String]) -> usize {
    let chars: Vec<char> = block.chars().collect();
    terms
        .iter()
        .filter(|term| !match_ranges(&chars, std::slice::from_ref(term)).is_empty())
        .count()
}

/// Turn one block into spans, windowed around its first match.
fn spans_for(block: &str, terms: &[String], max_chars: usize) -> Vec<Span> {
    let chars: Vec<char> = block.chars().collect();
    let ranges = match_ranges(&chars, terms);

    // Window: start a little before the first match so it is not flush left.
    let first = ranges.first().map_or(0, |(start, _)| *start);
    let mut start = first.saturating_sub(LEAD_IN);
    let mut end = (start + max_chars).min(chars.len());
    // Prefer whole words at both edges.
    while start > 0 && is_word(chars[start]) && is_word(chars[start - 1]) {
        start -= 1;
    }
    while end < chars.len() && is_word(chars[end]) && is_word(chars[end - 1]) {
        end += 1;
    }

    let mut spans: Vec<Span> = Vec::new();
    let mut push = |text: String, matched: bool| {
        if text.is_empty() {
            return;
        }
        // Merge into the previous span when the flag matches, so the frontend
        // never renders two adjacent identical elements.
        match spans.last_mut() {
            Some(last) if last.matched == matched => last.text.push_str(&text),
            _ => spans.push(Span { text, matched }),
        }
    };

    if start > 0 {
        push("…".to_owned(), false);
    }

    let mut cursor = start;
    for (match_start, match_end) in ranges {
        if match_end <= start {
            continue;
        }
        if match_start >= end {
            break;
        }
        let match_start = match_start.max(start);
        let match_end = match_end.min(end);
        push(chars[cursor..match_start].iter().collect(), false);
        push(chars[match_start..match_end].iter().collect(), true);
        cursor = match_end;
    }
    push(chars[cursor..end].iter().collect(), false);

    if end < chars.len() {
        push("…".to_owned(), false);
    }

    spans
}

/// Collect the page's text one block at a time.
///
/// Block boundaries are the whole point — see the module docs. A paragraph, a
/// heading, a list item and a table cell are each one block, and a snippet is
/// built from exactly one of them.
fn collect_blocks(node: &Node, out: &mut Vec<String>) {
    match node {
        Node::Paragraph { children }
        | Node::Heading { children, .. }
        | Node::Blockquote { children } => {
            let text = inline(children);
            if !text.is_empty() {
                out.push(text);
            }
        }

        Node::CodeBlock { code, .. } => {
            // Code is a block too: for a symbol query the example *is* the
            // answer, and showing prose instead would be showing the wrong
            // thing. Long blocks are windowed like any other.
            let trimmed = code.trim();
            if !trimmed.is_empty() {
                out.push(trimmed.to_owned());
            }
        }

        Node::List { items, .. } => {
            for item in items {
                for child in &item.children {
                    collect_blocks(child, out);
                }
            }
        }

        Node::DefinitionList { items } => {
            for item in items {
                // Term and definition together: on a Sphinx reference page the
                // term is the signature and the definition is what it does,
                // and either alone is half an answer.
                let mut text = inline(&item.term);
                let definition_blocks = {
                    let mut nested = Vec::new();
                    for child in &item.definition {
                        collect_blocks(child, &mut nested);
                    }
                    nested
                };
                if let Some(first) = definition_blocks.first() {
                    if !text.is_empty() {
                        text.push_str(" — ");
                    }
                    text.push_str(first);
                }
                if !text.is_empty() {
                    out.push(text);
                }
                for rest in definition_blocks.into_iter().skip(1) {
                    out.push(rest);
                }
            }
        }

        Node::Table { headers, rows } => {
            for cell in headers {
                for child in &cell.children {
                    collect_blocks(child, out);
                }
            }
            for row in rows {
                for cell in &row.cells {
                    for child in &cell.children {
                        collect_blocks(child, out);
                    }
                }
            }
        }

        Node::Admonition {
            title, children, ..
        } => {
            if let Some(title) = title {
                if !title.is_empty() {
                    out.push(title.clone());
                }
            }
            for child in children {
                collect_blocks(child, out);
            }
        }

        Node::Document { children } => {
            for child in children {
                collect_blocks(child, out);
            }
        }

        // Inline nodes reached at block level have no block of their own; a
        // bare `Text` outside a paragraph is a fragment, not a sentence.
        Node::Text { .. }
        | Node::InlineCode { .. }
        | Node::Emphasis { .. }
        | Node::Strong { .. }
        | Node::Link { .. }
        | Node::Image { .. }
        | Node::Anchor { .. }
        | Node::ThematicBreak {}
        | Node::LineBreak {} => {}
    }
}

/// Flatten inline children into one string.
fn inline(children: &[Node]) -> String {
    let mut out = String::new();
    for child in children {
        inline_into(child, &mut out);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn inline_into(node: &Node, out: &mut String) {
    match node {
        Node::Text { value } => out.push_str(value),
        Node::InlineCode { code } => out.push_str(code),
        Node::Emphasis { children } | Node::Strong { children } | Node::Link { children, .. } => {
            for child in children {
                inline_into(child, out);
            }
        }
        Node::LineBreak {} => out.push(' '),
        _ => {}
    }
}

/// The default snippet length, for callers that have no opinion.
pub const fn default_length() -> usize {
    DEFAULT_LENGTH
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn text(value: &str) -> Node {
        Node::Text {
            value: value.to_owned(),
        }
    }

    fn para(value: &str) -> Node {
        Node::Paragraph {
            children: vec![text(value)],
        }
    }

    fn document(children: Vec<Node>) -> Node {
        Node::Document { children }
    }

    fn terms(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    /// The snippet's visible text, spans reassembled.
    fn rendered(spans: &[Span]) -> String {
        spans.iter().map(|span| span.text.as_str()).collect()
    }

    fn matched(spans: &[Span]) -> Vec<&str> {
        spans
            .iter()
            .filter(|span| span.matched)
            .map(|span| span.text.as_str())
            .collect()
    }

    #[test]
    fn the_matching_block_is_chosen_not_the_first() {
        let doc = document(vec![
            para("An introduction that says nothing in particular."),
            para("Cargo reads environment variables at build time."),
        ]);
        let spans = snippet(&doc, &terms(&["environment"]), 240);
        assert!(rendered(&spans).contains("Cargo reads"), "{spans:?}");
        assert_eq!(matched(&spans), ["environment"]);
    }

    #[test]
    fn a_snippet_never_crosses_a_block_boundary() {
        // The reason this reads the stored tree rather than the flattened
        // field text: two adjacent paragraphs are not a sentence.
        let doc = document(vec![
            para("The first paragraph ends here."),
            para("The second begins with environment and continues."),
        ]);
        let spans = snippet(&doc, &terms(&["environment"]), 240);
        let out = rendered(&spans);
        assert!(!out.contains("ends here"), "{out:?}");
        assert!(out.contains("The second begins"), "{out:?}");
    }

    #[test]
    fn the_block_matching_the_most_distinct_terms_wins() {
        // Distinct, not total — otherwise a paragraph repeating one word beats
        // one that answers the whole query.
        let doc = document(vec![
            para("environment environment environment environment"),
            para("The environment variable is read at build time."),
        ]);
        let spans = snippet(&doc, &terms(&["environment", "variable"]), 240);
        assert!(
            rendered(&spans).contains("is read at build time"),
            "{spans:?}"
        );
    }

    #[test]
    fn matches_are_whole_words() {
        // Highlighting `cat` inside `concatenate` teaches the reader that the
        // highlight means nothing.
        let doc = document(vec![para("Concatenate the cat and the category.")]);
        let spans = snippet(&doc, &terms(&["cat"]), 240);
        assert_eq!(matched(&spans), ["cat"]);
        assert_eq!(rendered(&spans), "Concatenate the cat and the category.");
    }

    #[test]
    fn matching_is_case_insensitive_but_the_text_keeps_its_case() {
        let doc = document(vec![para("The Environment Variable ENVIRONMENT is set.")]);
        let spans = snippet(&doc, &terms(&["environment"]), 240);
        assert_eq!(matched(&spans), ["Environment", "ENVIRONMENT"]);
        assert_eq!(
            rendered(&spans),
            "The Environment Variable ENVIRONMENT is set."
        );
    }

    #[test]
    fn overlapping_terms_do_not_duplicate_text() {
        // Two terms can cover the same characters — a word and its correction,
        // say. Reassembled spans must still equal the source text.
        let source = "The environment is set.";
        let doc = document(vec![para(source)]);
        let spans = snippet(&doc, &terms(&["environment", "environment"]), 240);
        assert_eq!(rendered(&spans), source);
        assert_eq!(matched(&spans), ["environment"]);
    }

    #[test]
    fn a_page_with_no_match_still_gets_a_snippet() {
        // A result with no snippet reads as a broken result.
        let doc = document(vec![para("Nothing here is relevant to the query.")]);
        let spans = snippet(&doc, &terms(&["kubernetes"]), 240);
        assert_eq!(rendered(&spans), "Nothing here is relevant to the query.");
        assert!(matched(&spans).is_empty());
    }

    #[test]
    fn an_empty_page_yields_no_spans() {
        assert!(snippet(&document(vec![]), &terms(&["anything"]), 240).is_empty());
        assert!(snippet(&document(vec![]), &[], 240).is_empty());
    }

    #[test]
    fn a_long_block_is_windowed_around_the_match() {
        let filler = "padding words ".repeat(200);
        let doc = document(vec![para(&format!("{filler}environment variable{filler}"))]);
        let spans = snippet(&doc, &terms(&["environment"]), 120);
        let out = rendered(&spans);
        assert!(out.chars().count() < 260, "{} chars", out.chars().count());
        assert!(out.starts_with('…'), "{out:?}");
        assert!(out.ends_with('…'), "{out:?}");
        assert_eq!(matched(&spans), ["environment"]);
    }

    #[test]
    fn a_window_does_not_end_mid_word() {
        let doc = document(vec![para(
            "environment followed by supercalifragilisticexpialidocious and more",
        )]);
        let spans = snippet(&doc, &terms(&["environment"]), 40);
        let out = rendered(&spans).replace('…', "");
        // Every whitespace-separated piece that came from the source must be a
        // whole word from it.
        for word in out.split_whitespace() {
            assert!(
                "environment followed by supercalifragilisticexpialidocious and more"
                    .split_whitespace()
                    .any(|w| w == word),
                "{word:?} is a fragment of a word, from {out:?}"
            );
        }
    }

    #[test]
    fn multi_byte_text_is_sliced_by_character() {
        // Byte arithmetic here would panic on a multi-byte boundary.
        let doc = document(vec![para("日本語のテキスト environment 日本語のテキスト")]);
        let spans = snippet(&doc, &terms(&["environment"]), 240);
        assert_eq!(matched(&spans), ["environment"]);
        assert!(rendered(&spans).contains("日本語"));

        let emoji = document(vec![para("🎉🎉🎉 environment 🎉🎉🎉")]);
        assert_eq!(
            matched(&snippet(&emoji, &terms(&["environment"]), 240)),
            ["environment"]
        );
    }

    #[test]
    fn adjacent_spans_never_share_a_flag() {
        // Two adjacent unmatched spans would render as two elements for no
        // reason, and make the frontend's output depend on internal chunking.
        let doc = document(vec![para("alpha beta gamma delta epsilon")]);
        let spans = snippet(&doc, &terms(&["beta", "delta"]), 240);
        for pair in spans.windows(2) {
            assert_ne!(pair[0].matched, pair[1].matched, "{spans:?}");
        }
    }

    #[test]
    fn a_code_block_can_be_the_snippet() {
        // For a symbol query the example is the answer, and showing prose
        // instead would be showing the wrong thing.
        let doc = document(vec![
            para("This page is about reading files in general."),
            Node::CodeBlock {
                language: Some("rust".to_owned()),
                code: "let s = std::fs::read_to_string(path)?;".to_owned(),
            },
        ]);
        let spans = snippet(&doc, &terms(&["read_to_string"]), 240);
        assert!(rendered(&spans).contains("std::fs"), "{spans:?}");
        assert_eq!(matched(&spans), ["read_to_string"]);
    }

    #[test]
    fn list_items_and_table_cells_are_blocks() {
        let doc = document(vec![
            para("Opening prose."),
            Node::List {
                ordered: false,
                start: None,
                items: vec![crate::model::ListItem {
                    children: vec![para("A bullet mentioning environment.")],
                }],
            },
        ]);
        assert!(rendered(&snippet(&doc, &terms(&["environment"]), 240)).contains("A bullet"));

        let table = document(vec![Node::Table {
            headers: vec![crate::model::TableCell {
                children: vec![para("Variable")],
            }],
            rows: vec![crate::model::TableRow {
                cells: vec![crate::model::TableCell {
                    children: vec![para("CARGO_HOME describes the environment.")],
                }],
            }],
        }]);
        assert!(rendered(&snippet(&table, &terms(&["environment"]), 240)).contains("CARGO_HOME"));
    }

    #[test]
    fn inline_code_inside_a_paragraph_stays_in_its_block() {
        let doc = document(vec![Node::Paragraph {
            children: vec![
                text("Call "),
                Node::InlineCode {
                    code: "read_to_string".to_owned(),
                },
                text(" to read a file."),
            ],
        }]);
        let spans = snippet(&doc, &terms(&["read_to_string"]), 240);
        assert_eq!(rendered(&spans), "Call read_to_string to read a file.");
        assert_eq!(matched(&spans), ["read_to_string"]);
    }

    #[test]
    fn whitespace_is_collapsed() {
        // A stored tree can carry the source's line wrapping. Rendering it
        // verbatim puts ragged gaps in a one-line snippet.
        let doc = document(vec![para("lots   of\n\n  space   here")]);
        assert_eq!(rendered(&snippet(&doc, &[], 240)), "lots of space here");
    }
}
