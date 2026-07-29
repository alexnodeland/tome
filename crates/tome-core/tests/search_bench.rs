//! Search performance (S2-12, spec P2-018).
//!
//! ```bash
//! cargo test -p tome-core --test search_bench --release                       # the gate
//! cargo test -p tome-core --test search_bench --release -- --ignored --nocapture  # the report
//! ```
//!
//! # Two things, deliberately separated
//!
//! **`search_latency_meets_the_stage_2_exit_gate`** is a gate and runs
//! normally. It is the "P95 < 100 ms on the benchmark corpus" half of Stage
//! 2's exit criteria, measured over the real relevance corpus with the real
//! 207 queries — not synthetic ones, because a synthetic query has whatever
//! selectivity its author gave it.
//!
//! **`search_performance_report`** is ignored and prints a table: index build
//! time, query latency by query kind, and both at several index sizes, which
//! is the rest of what P2-018 asks for. It is a measurement for a person, not
//! an assertion.
//!
//! # Why there is no committed timing baseline
//!
//! Every other corpus here commits one — relevance, detection — and gates on
//! regression against it. **Timings cannot work that way.** A baseline
//! recorded on this laptop would fail on a slower one and pass on a faster one
//! while hiding a real regression, and the failure would look like a bug in
//! the code rather than a difference in the machine. Committing one would
//! produce a gate that gets suppressed within a month.
//!
//! So the gate is an *absolute* threshold with enormous headroom: the spec
//! says 100 ms and the measured figure is three orders of magnitude below it.
//! It fires on catastrophic regression — an accidental full scan, a lost
//! index — and on nothing else. That is a narrower promise than the other
//! corpora make, and it is the honest one.
//!
//! **Read the numbers from a release build.** A debug build measures rustc's
//! bounds checks — measured 2026-07-29, P95 is 158 µs in release and 1.47 ms
//! in debug. The *gate* passes in both, because the headroom swallows the
//! difference, which is why `check.sh` can run it without a release build and
//! without becoming flaky.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Deserialize;
use tome_core::model::{ContentHash, Node, Page, PagePath, SourceId};
use tome_core::search::SearchEngine;

/// The exit gate's number. From `docs/plans/18-implementation-plan.md`.
const P95_BUDGET: Duration = Duration::from_millis(100);

/// How many times each query runs. Enough for a P95 over 207 queries to mean
/// something, few enough that the gate stays under a second.
const REPETITIONS: usize = 5;

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/relevance")
}

#[derive(Deserialize)]
struct Corpus {
    sources: Vec<SourceEntry>,
}

#[derive(Deserialize)]
struct SourceEntry {
    id: String,
    category: String,
}

#[derive(Deserialize)]
struct QuerySet {
    queries: Vec<Query>,
}

#[derive(Deserialize)]
struct Query {
    kind: String,
    q: String,
}

struct Document {
    source: String,
    path: String,
    category: String,
    page: tome_core::store::StoredPage,
}

fn load_documents() -> Vec<Document> {
    let dir = corpus_dir();
    let corpus: Corpus = serde_yaml_ng::from_str(
        &std::fs::read_to_string(dir.join("corpus.yaml")).expect("read corpus.yaml"),
    )
    .expect("parse corpus.yaml");

    let root = dir.join("pages");
    let mut documents = Vec::new();
    for source in &corpus.sources {
        let mut stack = vec![root.join(&source.id)];
        while let Some(current) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&current) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "json") {
                    continue;
                }
                let page: tome_core::store::StoredPage =
                    serde_json::from_str(&std::fs::read_to_string(&path).expect("read page"))
                        .expect("parse page");
                documents.push(Document {
                    source: source.id.clone(),
                    path: page.path.as_str().to_owned(),
                    category: source.category.clone(),
                    page,
                });
            }
        }
    }
    documents
}

fn load_queries() -> Vec<Query> {
    let raw =
        std::fs::read_to_string(corpus_dir().join("queries.yaml")).expect("read queries.yaml");
    serde_yaml_ng::from_str::<QuerySet>(&raw)
        .expect("parse queries.yaml")
        .queries
}

/// Build an index from the corpus, returning it and how long that took.
fn build(dir: &Path, documents: &[Document]) -> (SearchEngine, Duration) {
    let started = Instant::now();
    let engine = SearchEngine::open_at(dir).expect("open index");
    let mut session = engine.session().expect("session");
    for document in documents {
        let meta = Page::new(
            SourceId::new(&document.source).expect("source id"),
            PagePath::new(&document.path).expect("page path"),
            document.page.title.clone(),
            ContentHash::new("0".repeat(64)).expect("hash"),
        );
        session
            .add_page(&meta, &document.category, &document.page.body)
            .expect("index page");
    }
    session.commit().expect("commit");
    drop(session);
    (engine, started.elapsed())
}

/// Percentile from an unsorted sample. `sorted` is sorted in place.
fn percentile(sorted: &mut [Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    sorted.sort_unstable();
    // Nearest-rank, which for a sample this size is both the simplest
    // definition and the conservative one: it never reports a value the
    // system did not actually produce.
    let rank = ((p / 100.0) * sorted.len() as f64).ceil().max(1.0) as usize;
    sorted[rank.min(sorted.len()) - 1]
}

#[test]
fn search_latency_meets_the_stage_2_exit_gate() {
    let documents = load_documents();
    let queries = load_queries();
    assert!(documents.len() >= 150, "the corpus floor is 150 documents");
    assert!(queries.len() >= 200, "P2-019 requires at least 200 queries");

    let dir = tempfile::tempdir().expect("tempdir");
    let (engine, _) = build(dir.path(), &documents);

    // Warm: the first search of a freshly opened index pays for mmap faults
    // that no later one does, and a P95 over 207 queries would otherwise be
    // measuring page-in cost once.
    for query in queries.iter().take(20) {
        let _ = engine.search(&query.q, 10);
    }

    let mut samples = Vec::with_capacity(queries.len() * REPETITIONS);
    for _ in 0..REPETITIONS {
        for query in &queries {
            let started = Instant::now();
            let _ = engine.search(&query.q, 10);
            samples.push(started.elapsed());
        }
    }

    let p50 = percentile(&mut samples.clone(), 50.0);
    // `percentile` sorts in place, so `samples` is ascending afterwards and
    // its last element is the maximum. Spelled out because relying on a
    // helper's side effect is otherwise the kind of thing that breaks when
    // the helper stops sorting.
    let p95 = percentile(&mut samples, 95.0);
    let max = samples.last().copied().unwrap_or_default();

    println!(
        "\nsearch latency over {} documents, {} queries × {REPETITIONS}\n  \
         p50 {p50:?}\n  p95 {p95:?}\n  max {max:?}\n  budget {P95_BUDGET:?}",
        documents.len(),
        queries.len(),
    );

    assert!(
        p95 < P95_BUDGET,
        "Stage 2's exit gate is P95 < {P95_BUDGET:?} on the benchmark corpus; measured {p95:?}. \
         This threshold has three orders of magnitude of headroom, so it fires on a lost index \
         or an accidental full scan rather than on a slow machine."
    );
}

#[test]
fn indexing_a_corpus_stays_far_cheaper_than_crawling_it() {
    // SPIKE-003 finding 1, kept honest: indexing is three orders of magnitude
    // cheaper than crawling, which is the entire justification for putting the
    // index in the cache and rebuilding it rather than treating it as
    // precious. If that ever stops being true, `open_or_rebuild` becomes a
    // destructive operation rather than a cheap one.
    let documents = load_documents();
    let dir = tempfile::tempdir().expect("tempdir");
    let (_, elapsed) = build(dir.path(), &documents);

    let per_page = elapsed / u32::try_from(documents.len()).unwrap_or(1);
    println!(
        "\nindexed {} documents in {elapsed:?} ({per_page:?} per page)",
        documents.len()
    );

    // A polite crawl is rate-limited to a few requests a second, so a page
    // costs hundreds of milliseconds at best. Ten milliseconds of indexing per
    // page would still be an order of magnitude cheaper; anything above it
    // means something has gone badly wrong.
    assert!(
        per_page < Duration::from_millis(10),
        "indexing cost {per_page:?} per page. SPIKE-003 measured this three orders of \
         magnitude below the cost of fetching one."
    );
}

/// P2-018's report: latency by query kind, and at several index sizes.
///
/// `cargo test -p tome-core --test search_bench --release -- --ignored --nocapture`
///
/// Ignored because it is a measurement rather than an assertion, and because
/// the larger sizes take long enough to be rude in a gate.
#[test]
#[ignore = "a measurement, not a gate: run it by hand when changing the index"]
fn search_performance_report() {
    let documents = load_documents();
    let queries = load_queries();

    println!("\nsearch performance\n──────────────────");

    // ---- by index size. Larger sizes repeat the corpus under distinct source
    // ids, which grows the *posting lists* honestly while leaving the
    // vocabulary alone. SPIKE-003 finding 2 is that index size tracks
    // vocabulary, so this measures the axis repetition actually moves and the
    // report says so rather than implying it is a full-scale test.
    println!(
        "\n  {:<12} {:>12} {:>12} {:>12}",
        "documents", "build", "p50", "p95"
    );
    for multiple in [1usize, 4, 16] {
        let mut grown = Vec::with_capacity(documents.len() * multiple);
        for copy in 0..multiple {
            for document in &documents {
                grown.push(Document {
                    source: format!("{}-{copy}", document.source),
                    path: document.path.clone(),
                    category: document.category.clone(),
                    page: document.page.clone(),
                });
            }
        }

        let dir = tempfile::tempdir().expect("tempdir");
        let (engine, build_time) = build(dir.path(), &grown);
        for query in queries.iter().take(20) {
            let _ = engine.search(&query.q, 10);
        }

        let mut samples: Vec<Duration> = queries
            .iter()
            .map(|query| {
                let started = Instant::now();
                let _ = engine.search(&query.q, 10);
                started.elapsed()
            })
            .collect();
        let p50 = percentile(&mut samples.clone(), 50.0);
        let p95 = percentile(&mut samples, 95.0);
        println!(
            "  {:<12} {:>12?} {:>12?} {:>12?}",
            grown.len(),
            build_time,
            p50,
            p95
        );
    }

    // ---- by query kind, at the corpus's real size.
    let dir = tempfile::tempdir().expect("tempdir");
    let (engine, _) = build(dir.path(), &documents);
    for query in queries.iter().take(20) {
        let _ = engine.search(&query.q, 10);
    }

    let mut by_kind: std::collections::BTreeMap<&str, Vec<Duration>> =
        std::collections::BTreeMap::new();
    for _ in 0..REPETITIONS {
        for query in &queries {
            let started = Instant::now();
            let _ = engine.search(&query.q, 10);
            by_kind
                .entry(query.kind.as_str())
                .or_default()
                .push(started.elapsed());
        }
    }

    println!(
        "\n  {:<14} {:>6} {:>12} {:>12}",
        "query kind", "n", "p50", "p95"
    );
    for (kind, mut samples) in by_kind {
        let n = samples.len();
        let p50 = percentile(&mut samples.clone(), 50.0);
        let p95 = percentile(&mut samples, 95.0);
        println!("  {kind:<14} {n:>6} {p50:>12?} {p95:>12?}");
    }

    println!(
        "\n  `misspelling` is the row to watch: it is the only kind that triggers a term\n  \
         dictionary scan (S2-5), and its cost tracks vocabulary rather than page count.\n  \
         SPIKE-003's 100k-page figures are the ones that matter for scale; this corpus\n  \
         cannot reproduce them and does not claim to."
    );
}

/// The synthetic-page generator P2-018's sketch uses, kept honest.
///
/// Not used by the report above, and that is the point: a benchmark over
/// generated pages measures the generator's idea of a document. Every number
/// here comes from real pages and real queries. This exists only so the choice
/// is visible rather than looking like an omission.
#[allow(dead_code)]
fn generated_page(n: usize) -> Node {
    Node::Document {
        children: vec![Node::Paragraph {
            children: vec![Node::Text {
                value: format!("synthetic page {n}"),
            }],
        }],
    }
}
