//! Search relevance evaluation (S2-1, spec P2-019).
//!
//! Ranking changes are otherwise unmeasurable. Without this, a boost tweak is
//! an opinion, and with agents writing the tweaks it is *fast* opinion —
//! twenty confident changes and no way to tell which helped. This converts
//! that into a gradient.
//!
//! ```bash
//! cargo test -p tome-core --test relevance -- --nocapture   # see the report
//! TOME_UPDATE_BASELINE=1 cargo test -p tome-core --test relevance
//! git diff -- crates/tome-core/corpus/relevance/baseline.json
//! ```
//!
//! Update mode **fails the run it changes anything in**, deliberately, the
//! same way `TOME_UPDATE_GOLDEN` does: the passing run is the one after the
//! diff has been looked at. A baseline that can be silently rewritten measures
//! nothing.
//!
//! Everything is offline and deterministic: the index is built from the
//! committed normalization goldens, so no HTML is parsed and no network is
//! touched.
//!
//! # How much this gate is worth, measured
//!
//! **It is a strong diagnostic and a weak gate**, and that is not a guess —
//! three perturbations were run against it when it was built:
//!
//! | Change | MRR | Queries moved | Gate fires? |
//! |---|---|---|---|
//! | Title boost 3.0 → 0.05 (60×) | 0.9168 → 0.9156 | 4 worse, 4 better | no |
//! | …and code boost 1.5 → 0.001 (1500×) | 0.9168 → 0.9210 | 8 worse, 10 better | no |
//! | Remove `code` from the query entirely | 0.9168 → 0.9132 | 9 worse, 8 better | no |
//!
//! Deleting a whole indexed field from search moved the aggregate by 0.0036.
//! The reason is the corpus: with 26 documents there is usually no strong
//! wrong answer for a better-ranked right answer to beat, so the metrics
//! compress into a narrow band near the top.
//!
//! Two consequences, both load-bearing:
//!
//! - **Do not read a passing run as "ranking is fine."** It means nothing
//!   catastrophic happened. S2-4 cannot be scored against this corpus as it
//!   stands; making boost tuning measurable needs materially more documents,
//!   which is a corpus decision (fetching and licence-verifying more pages),
//!   not a harness one.
//! - **The per-query movement report is the part that works.** It caught every
//!   one of those three perturbations. Read it, rather than the aggregate.
//!
//! What this *did* catch, on its very first run, was a real defect: twelve
//! symbol queries returning nothing because `()` and `[]` are query-parser
//! syntax. Fixing that moved symbol recall@1 from 0.7465 to 0.9474. Finding
//! defects is what this is good at; policing small ranking changes is not,
//! yet.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tome_core::model::{ContentHash, Page, PagePath, SourceId};
use tome_core::normalize::NormalizedPage;
use tome_core::search::SearchEngine;

/// How deep a hit still counts. Also the depth `recall@10` reports, and the
/// cutoff below which a rank is recorded as "not found" (0).
const DEPTH: usize = 10;

/// How far an aggregate metric may fall before the run fails.
///
/// Not zero: BM25 scores can reorder tied documents when unrelated content
/// changes, and a gate that fires on noise gets suppressed. 0.02 is roughly
/// four of these 207 queries moving between rank 1 and rank 2 — small enough
/// to catch a real ranking change, large enough not to fire on a reworded
/// heading in one corpus page.
const MARGIN: f64 = 0.02;

/// How many more queries may get *worse* than get better before the run fails,
/// independently of the aggregate.
///
/// This second gate exists because the first one is weak, and the eval set
/// demonstrated it: cutting the title boost 60× (3.0 → 0.05) moved MRR by
/// 0.0012 — a sixteenth of `MARGIN` — while visibly reshuffling eight queries.
/// On a 26-document corpus the aggregates are simply not sensitive enough to
/// police ranking changes on their own, because there is rarely a strong wrong
/// answer to be beaten by.
///
/// Net movement is the sensitive signal, so it gets its own threshold. It is
/// *net* rather than absolute because BM25 reorders ties freely: a change that
/// moves four queries down and four up has not degraded anything, and a gate
/// that fired on that would be suppressed within a week.
///
/// Set to 3 rather than something tighter because the three perturbations in
/// the module docs produced net movements of 0, 0, and 1 — so this threshold
/// is *also* not sensitive enough to catch them. It is here to catch
/// directional damage the aggregate smooths over, not to substitute for a
/// corpus large enough to discriminate.
const MAX_NET_DEGRADED: usize = 3;

// --------------------------------------------------------------- the inputs

#[derive(Debug, Deserialize)]
struct Corpus {
    documents: Vec<DocumentEntry>,
}

#[derive(Debug, Deserialize)]
struct DocumentEntry {
    /// Base name of the golden in `corpus/normalization/golden/`.
    file: String,
    source: String,
    path: String,
    category: String,
}

impl DocumentEntry {
    /// `source:path` — how query labels name a document.
    fn key(&self) -> String {
        format!("{}:{}", self.source, self.path)
    }
}

#[derive(Debug, Deserialize)]
struct QuerySet {
    queries: Vec<Query>,
}

#[derive(Debug, Deserialize)]
struct Query {
    id: String,
    kind: String,
    q: String,
    /// Any one of these being top-ranked is correct. See the note in
    /// `queries.yaml`: insisting on a single gold document would penalise a
    /// ranker for being right.
    want: Vec<String>,
}

// -------------------------------------------------------------- the results

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
struct Baseline {
    metrics: Metrics,
    /// Query id → rank of the first acceptable hit, 1-based. `0` means the
    /// query found nothing acceptable within `DEPTH`.
    ///
    /// A `BTreeMap` so the committed file has a stable order and its diffs are
    /// readable; a `HashMap` would reshuffle the whole file on every update.
    ranks: BTreeMap<String, usize>,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
struct Metrics {
    mrr: f64,
    recall_at_1: f64,
    recall_at_3: f64,
    recall_at_10: f64,
}

impl Metrics {
    fn from_ranks(ranks: &BTreeMap<String, usize>) -> Self {
        let n = ranks.len();
        if n == 0 {
            return Self::default();
        }
        let total = n as f64;
        let hits_within =
            |k: usize| ranks.values().filter(|r| **r >= 1 && **r <= k).count() as f64 / total;
        Self {
            mrr: ranks
                .values()
                .map(|r| if *r >= 1 { 1.0 / *r as f64 } else { 0.0 })
                .sum::<f64>()
                / total,
            recall_at_1: hits_within(1),
            recall_at_3: hits_within(3),
            recall_at_10: hits_within(DEPTH),
        }
    }

    fn named(&self) -> [(&'static str, f64); 4] {
        [
            ("MRR", self.mrr),
            ("recall@1", self.recall_at_1),
            ("recall@3", self.recall_at_3),
            ("recall@10", self.recall_at_10),
        ]
    }
}

// ---------------------------------------------------------------- the index

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus")
}

/// Build the eval index from the committed normalization goldens.
fn build_index(dir: &Path, documents: &[DocumentEntry]) -> SearchEngine {
    let engine = SearchEngine::open_at(dir).expect("open eval index");
    let goldens = corpus_dir().join("normalization/golden");

    let mut session = engine.session().expect("index session");
    for entry in documents {
        let file = goldens.join(format!("{}.json", entry.file));
        let raw = std::fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("read golden {}: {e}", file.display()));
        let page: NormalizedPage = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("parse golden {}: {e}", file.display()));

        let meta = Page::new(
            SourceId::new(&entry.source).expect("source id"),
            PagePath::new(&entry.path).expect("page path"),
            page.title.clone().unwrap_or_default(),
            // The eval never change-detects, so any valid hash will do.
            ContentHash::new("0".repeat(64)).expect("hash"),
        );

        session
            .add_page(&meta, &entry.category, &page.body)
            .expect("index page");
    }
    session.commit().expect("commit eval index");
    drop(session);

    engine
}

/// Rank of the first acceptable hit, 1-based; 0 if none within `DEPTH`.
fn rank_of(engine: &SearchEngine, query: &Query) -> usize {
    // A query the parser rejects scores 0 rather than failing the run: the
    // eval set deliberately contains user-shaped input, and "this query is
    // unparseable" is a relevance result, not a harness error.
    let Ok(hits) = engine.search(&query.q, DEPTH) else {
        return 0;
    };
    hits.iter()
        .position(|hit| {
            let key = format!("{}:{}", hit.source.as_str(), hit.path);
            query.want.contains(&key)
        })
        .map_or(0, |index| index + 1)
}

// ----------------------------------------------------------------- the test

#[test]
fn relevance_does_not_regress() {
    let dir = corpus_dir().join("relevance");

    let corpus: Corpus = serde_yaml_ng::from_str(
        &std::fs::read_to_string(dir.join("corpus.yaml")).expect("read corpus.yaml"),
    )
    .expect("parse corpus.yaml");

    let queries: QuerySet = serde_yaml_ng::from_str(
        &std::fs::read_to_string(dir.join("queries.yaml")).expect("read queries.yaml"),
    )
    .expect("parse queries.yaml");

    // P2-019's floor, asserted rather than assumed: a corpus that quietly
    // shrank below this would still produce plausible-looking metrics.
    assert!(
        queries.queries.len() >= 200,
        "P2-019 requires at least 200 queries, found {}",
        queries.queries.len()
    );
    let sources: std::collections::BTreeSet<_> =
        corpus.documents.iter().map(|d| &d.source).collect();
    assert!(
        sources.len() >= 5,
        "P2-019 requires at least 5 fixture sources, found {}",
        sources.len()
    );

    // Every label must name a real document. A typo here would silently score
    // as "not found" forever and read as a permanent ranking failure.
    let known: std::collections::BTreeSet<String> =
        corpus.documents.iter().map(DocumentEntry::key).collect();
    let mut unlabelled = Vec::new();
    for query in &queries.queries {
        for want in &query.want {
            if !known.contains(want) {
                unlabelled.push(format!("{} → {want}", query.id));
            }
        }
    }
    assert!(
        unlabelled.is_empty(),
        "queries name documents that are not in corpus.yaml:\n  {}",
        unlabelled.join("\n  ")
    );

    let index_dir = tempfile::tempdir().expect("tempdir");
    let engine = build_index(index_dir.path(), &corpus.documents);
    assert_eq!(
        engine.len().expect("doc count"),
        corpus.documents.len() as u64,
        "every corpus document should be indexed"
    );

    let ranks: BTreeMap<String, usize> = queries
        .queries
        .iter()
        .map(|query| (query.id.clone(), rank_of(&engine, query)))
        .collect();
    let metrics = Metrics::from_ranks(&ranks);
    let current = Baseline { metrics, ranks };

    let baseline_path = dir.join("baseline.json");
    let updating = std::env::var_os("TOME_UPDATE_BASELINE").is_some();

    let report = report(&current, &queries.queries);

    if !baseline_path.exists() {
        write_baseline(&baseline_path, &current);
        panic!(
            "{report}\nNo baseline existed; wrote {}. Review it and re-run.",
            baseline_path.display()
        );
    }

    let previous: Baseline =
        serde_json::from_str(&std::fs::read_to_string(&baseline_path).expect("read baseline.json"))
            .expect("parse baseline.json");

    let moved = movement(&previous, &current, &queries.queries);

    if updating {
        if previous == current {
            println!("{report}\nBaseline unchanged.");
            return;
        }
        write_baseline(&baseline_path, &current);
        panic!(
            "{}\n{report}\nBaseline rewritten. Review the diff, then re-run without \
             TOME_UPDATE_BASELINE.",
            moved.report
        );
    }

    let mut failures: Vec<String> = previous
        .metrics
        .named()
        .iter()
        .zip(current.metrics.named())
        .filter(|((_, was), (_, now))| was - now > MARGIN)
        .map(|((name, was), (_, now))| format!("{name}: {was:.4} → {now:.4} (margin {MARGIN})"))
        .collect();

    if moved.net_degraded() > MAX_NET_DEGRADED {
        failures.push(format!(
            "{} queries got worse and {} got better — a net {} degraded (limit {MAX_NET_DEGRADED})",
            moved.worse,
            moved.better,
            moved.net_degraded(),
        ));
    }

    assert!(
        failures.is_empty(),
        "{report}\n{}\nRelevance regressed:\n  {}\n\n\
         If this change is intended, re-run with TOME_UPDATE_BASELINE=1 and commit the \
         new baseline with the reason.",
        moved.report,
        failures.join("\n  "),
    );

    println!("{report}\n{}", moved.report);
}

fn write_baseline(path: &Path, baseline: &Baseline) {
    let json = serde_json::to_string_pretty(baseline).expect("serialize baseline");
    std::fs::write(path, format!("{json}\n")).expect("write baseline");
}

/// The aggregate table, plus a per-`kind` breakdown.
///
/// The breakdown is what makes a regression legible: "fuzzy matching got
/// worse" is actionable, a single number moving is not.
fn report(current: &Baseline, queries: &[Query]) -> String {
    let mut out = String::from("\nrelevance\n─────────\n");
    for (name, value) in current.metrics.named() {
        let _ = writeln!(out, "  {name:<11} {value:.4}");
    }

    let mut by_kind: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for query in queries {
        if let Some(rank) = current.ranks.get(&query.id) {
            by_kind.entry(&query.kind).or_default().push(*rank);
        }
    }

    let _ = writeln!(out, "\n  by kind          n   recall@1  recall@3       MRR");
    for (kind, ranks) in by_kind {
        let n = ranks.len() as f64;
        let within = |k: usize| ranks.iter().filter(|r| **r >= 1 && **r <= k).count() as f64 / n;
        let mrr = ranks
            .iter()
            .map(|r| if *r >= 1 { 1.0 / *r as f64 } else { 0.0 })
            .sum::<f64>()
            / n;
        let _ = writeln!(
            out,
            "  {kind:<13} {:>4}     {:.4}    {:.4}    {:.4}",
            ranks.len(),
            within(1),
            within(3),
            mrr
        );
    }
    out
}

/// Per-query movement, and the counts the second gate reads.
struct Movement {
    worse: usize,
    better: usize,
    report: String,
}

impl Movement {
    /// How many more queries degraded than improved.
    fn net_degraded(&self) -> usize {
        self.worse.saturating_sub(self.better)
    }
}

/// Which queries moved, and how. P2-019 asks for this by name — an aggregate
/// that fell tells you nothing about what to look at.
fn movement(previous: &Baseline, current: &Baseline, queries: &[Query]) -> Movement {
    let text = |id: &str| {
        queries
            .iter()
            .find(|q| q.id == id)
            .map_or(String::new(), |q| q.q.clone())
    };
    let show = |rank: usize| {
        if rank == 0 {
            "—".to_owned()
        } else {
            rank.to_string()
        }
    };

    let mut worse = Vec::new();
    let mut better = Vec::new();
    let mut added = Vec::new();

    for (id, now) in &current.ranks {
        match previous.ranks.get(id) {
            None => added.push(format!("    + {id}  ({})  rank {}", text(id), show(*now))),
            Some(was) if was == now => {}
            Some(was) => {
                // Rank 0 is "not found", so it is WORSE than any real rank
                // despite being numerically smallest. Comparing the raw
                // numbers would report a query falling out of the results
                // entirely as an improvement.
                let degraded = *now == 0 || (*was != 0 && now > was);
                let line = format!("    {id}  ({})  {} → {}", text(id), show(*was), show(*now));
                if degraded {
                    worse.push(line);
                } else {
                    better.push(line);
                }
            }
        }
    }
    let removed: Vec<String> = previous
        .ranks
        .keys()
        .filter(|id| !current.ranks.contains_key(*id))
        .map(|id| format!("    - {id}"))
        .collect();

    let counts = (worse.len(), better.len());

    if worse.is_empty() && better.is_empty() && added.is_empty() && removed.is_empty() {
        return Movement {
            worse: 0,
            better: 0,
            report: "  no query changed rank".to_owned(),
        };
    }

    let mut out = String::new();
    for (label, group) in [
        ("worse", &worse),
        ("better", &better),
        ("new", &added),
        ("removed", &removed),
    ] {
        if !group.is_empty() {
            let _ = writeln!(out, "  {label} ({}):", group.len());
            for line in group {
                let _ = writeln!(out, "{line}");
            }
        }
    }
    Movement {
        worse: counts.0,
        better: counts.1,
        report: out,
    }
}
