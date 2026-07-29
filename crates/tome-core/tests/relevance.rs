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
//! Everything is offline and deterministic: the index is built from 339
//! committed pages under `corpus/relevance/pages/`, each one a `StoredPage`
//! exactly as `pipeline::pull` wrote it, so no HTML is parsed and no network
//! is touched.
//!
//! # How much this gate is worth, measured
//!
//! The corpus was 26 documents when this harness was first written, and at
//! that size **it could not discriminate at all**: cutting the title boost
//! 60×, additionally cutting the code boost 1500×, and removing the `code`
//! field from the query entirely each moved MRR by ≤ 0.0036 and tripped
//! nothing. With 26 documents there is rarely a strong wrong answer for a
//! better-ranked right answer to beat.
//!
//! It was expanded to 339 documents for that reason. Re-running the
//! perturbations at the new size:
//!
//! | Change | MRR | Queries moved | Gate fires? |
//! |---|---|---|---|
//! | Title boost 3.0 → 0.05 (60×) | 0.7489 → 0.7625 | 17 worse, 28 better | no — net **improvement** |
//! | Remove `code` from the query | 0.7489 → 0.7536 | 13 worse, 15 better | no — a genuine wash |
//! | Search `body` only | 0.7489 → **0.4293** | 118 worse, 27 better | **yes**, both gates |
//!
//! That is the behaviour wanted: decisive on real damage, silent on changes
//! that are neutral or positive. The first two rows were not gate failures,
//! they were **findings**, and S2-4 acted on both — the title boost came down
//! from 3.0 to 0.75 and the code boost from 1.5 to 1.0.
//!
//! What the eval set caught on its very first run was a real defect: twelve
//! symbol queries returning nothing because `()` and `[]` are query-parser
//! syntax. Fixing that moved symbol recall@1 from 0.7465 to 0.9474 on the old
//! corpus.
//!
//! # What S2-4 and S2-5 changed, measured
//!
//! `sweep_ranking_parameters` below is the tool; [`Ranking::TUNED`] holds what
//! it found. Against the untuned ranker, over the same 207 queries:
//!
//! | | before S2-4 | after S2-4 | after S2-5 |
//! |---|---|---|---|
//! | MRR | 0.7489 | 0.8245 | 0.8351 |
//! | recall@1 | 0.6377 | 0.7585 | 0.7585 |
//! | recall@3 | 0.8357 | 0.8744 | 0.8986 |
//! | `natural` MRR | 0.2419 | 0.4596 | 0.4273 |
//! | `symbol` MRR | 0.8384 | 0.8792 | 0.8817 |
//! | `misspelling` MRR | 0.3125 | 0.3500 | 0.6333 |
//!
//! S2-4 was field boosts, a document-length penalty and query-time stopwords;
//! S2-5 was typo tolerance. Turning typo tolerance on moved **only**
//! `misspelling` queries, which is the behaviour wanted from it.
//!
//! # What is still weak
//!
//! **recall@3 is 0.8986 against a Stage 2 exit gate of 0.90 — one query short
//! of 207.** A neighbouring configuration reaches 0.9034, and taking it would
//! be fitting the gate rather than passing it; see [`Ranking::TUNED`].
//!
//! `natural` (0.4273 MRR) is now the weakest category. Two of its queries want
//! a page that answers a "how do I …" question in prose, and the pages that
//! do are the enormous single-page ones the length penalty exists to demote —
//! so it is a genuine tension rather than a bug, and closing it probably needs
//! passage-level retrieval rather than another boost.
//!
//! Two `misspelling` queries are unreachable by design: `modual` -> `module`
//! is 2 edits on a 6-character term and `pth` -> `path` is 1 edit on a
//! 3-character term, and P2-009's schedule allows 1 and 0 respectively.
//! Widening it is a specification change, not a tuning decision.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tome_core::model::{ContentHash, Page, PagePath, SourceId};
use tome_core::search::{Ranking, SearchEngine, StopwordPolicy};

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
/// This second gate exists because an aggregate can hide directional damage:
/// a change that helps twenty easy queries slightly and breaks five important
/// ones can leave MRR flat. Net movement sees that; the mean does not.
///
/// It is *net* rather than absolute because BM25 reorders ties freely — a
/// change that moves four queries down and four up has degraded nothing, and a
/// gate that fired on that would be suppressed within a week.
///
/// 3 is deliberately loose. The measured perturbations (module docs) produced
/// net movements of 0, 0, and 91: real damage clears this threshold by an
/// order of magnitude, so tightening it would only buy false positives.
const MAX_NET_DEGRADED: usize = 3;

// --------------------------------------------------------------- the inputs

#[derive(Debug, Deserialize)]
struct Corpus {
    sources: Vec<SourceEntry>,
}

/// One source's metadata. The *documents* are discovered by walking
/// `pages/<id>/`, not listed here — 339 hand-maintained entries would be a
/// second thing to keep in sync with the directory, and it would drift.
#[derive(Debug, Deserialize)]
struct SourceEntry {
    id: String,
    category: String,
}

/// One indexable document, loaded from `pages/<source>/<path>.json`.
struct Document {
    source: String,
    /// Recovered from the file's own `path` field rather than from its
    /// location on disk: a `StoredPage` names itself, and trusting the
    /// filesystem would let a case-insensitive volume rename a page silently.
    path: String,
    category: String,
    page: tome_core::store::StoredPage,
}

impl Document {
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

/// Load every document under `pages/<source>/`.
///
/// Walking the tree rather than reading a manifest is deliberate: a manifest
/// of 339 entries is a second copy of the directory listing, and the two
/// would drift. `corpus.yaml` records only what the tree cannot say.
fn load_documents(sources: &[SourceEntry]) -> Vec<Document> {
    let root = corpus_dir().join("relevance/pages");
    let mut documents = Vec::new();

    for source in sources {
        let dir = root.join(&source.id);
        let mut stack = vec![dir.clone()];
        let mut found = 0usize;

        while let Some(current) = stack.pop() {
            let entries = std::fs::read_dir(&current)
                .unwrap_or_else(|e| panic!("read {}: {e}", current.display()));
            for entry in entries {
                let path = entry.expect("dir entry").path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "json") {
                    continue;
                }
                let raw = std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
                let page: tome_core::store::StoredPage = serde_json::from_str(&raw)
                    .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
                documents.push(Document {
                    source: source.id.clone(),
                    path: page.path.as_str().to_owned(),
                    category: source.category.clone(),
                    page,
                });
                found += 1;
            }
        }
        assert!(found > 0, "source {} has no pages", source.id);
    }
    documents
}

/// Build the eval index from the committed pages.
fn build_index(dir: &Path, documents: &[Document]) -> SearchEngine {
    let engine = SearchEngine::open_at(dir).expect("open eval index");

    let mut session = engine.session().expect("index session");
    for document in documents {
        let meta = Page::new(
            SourceId::new(&document.source).expect("source id"),
            PagePath::new(&document.path).expect("page path"),
            document.page.title.clone(),
            // The eval never change-detects, so any valid hash will do.
            ContentHash::new("0".repeat(64)).expect("hash"),
        );

        session
            .add_page(&meta, &document.category, &document.page.body)
            .expect("index page");
    }
    session.commit().expect("commit eval index");
    drop(session);

    engine
}

/// Print what actually ranked top for queries that scored badly.
///
/// `TOME_RELEVANCE_DUMP=1`. This exists because a low score has two causes
/// that look identical in the aggregate — the ranking is wrong, or the label
/// is incomplete because some *other* page answers the query at least as well.
/// On a 339-document corpus the second is common, and the only way to tell
/// them apart is to read the results.
fn dump_poor_results(engine: &SearchEngine, queries: &[Query], ranks: &BTreeMap<String, usize>) {
    if std::env::var_os("TOME_RELEVANCE_DUMP").is_none() {
        return;
    }
    for query in queries {
        let rank = ranks.get(&query.id).copied().unwrap_or(0);
        if (1..=3).contains(&rank) {
            continue;
        }
        let hits = engine.search(&query.q, 5).unwrap_or_default();
        println!(
            "\n{} [{}] {:?}  rank={}",
            query.id,
            query.kind,
            query.q,
            if rank == 0 {
                "—".to_owned()
            } else {
                rank.to_string()
            }
        );
        println!("  want: {}", query.want.join(", "));
        for (i, hit) in hits.iter().enumerate() {
            println!(
                "  {}. {}:{}  ({})",
                i + 1,
                hit.source.as_str(),
                hit.path,
                hit.title
            );
        }
    }
}

/// Rank of the first acceptable hit, 1-based; 0 if none within `DEPTH`.
fn rank_of(engine: &SearchEngine, query: &Query, ranking: &Ranking) -> usize {
    // A query the parser rejects scores 0 rather than failing the run: the
    // eval set deliberately contains user-shaped input, and "this query is
    // unparseable" is a relevance result, not a harness error.
    let Ok(hits) = engine.search_with(&query.q, DEPTH, ranking) else {
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

    let documents = load_documents(&corpus.sources);

    // P2-019's floors, and the Stage 2 exit gate's, asserted rather than
    // assumed: a corpus that quietly shrank would still produce
    // plausible-looking metrics — which is precisely how this measurement
    // stops meaning anything.
    assert!(
        queries.queries.len() >= 200,
        "P2-019 requires at least 200 queries, found {}",
        queries.queries.len()
    );
    assert!(
        corpus.sources.len() >= 5,
        "P2-019 requires at least 5 fixture sources, found {}",
        corpus.sources.len()
    );
    assert!(
        documents.len() >= 150,
        "the Stage 2 exit gate requires at least 150 documents, found {} — \
         below this the metrics stop discriminating (see the module docs)",
        documents.len()
    );

    // Every label must name a real document. A typo here would silently score
    // as "not found" forever and read as a permanent ranking failure.
    let known: std::collections::BTreeSet<String> = documents.iter().map(Document::key).collect();
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
        "queries name documents that are not in the corpus:\n  {}",
        unlabelled.join("\n  ")
    );

    let index_dir = tempfile::tempdir().expect("tempdir");
    let engine = build_index(index_dir.path(), &documents);
    assert_eq!(
        engine.len().expect("doc count"),
        documents.len() as u64,
        "every corpus document should be indexed"
    );

    let ranks: BTreeMap<String, usize> = queries
        .queries
        .iter()
        .map(|query| (query.id.clone(), rank_of(&engine, query, &Ranking::TUNED)))
        .collect();
    let metrics = Metrics::from_ranks(&ranks);
    dump_poor_results(&engine, &queries.queries, &ranks);
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

// ----------------------------------------------------------------- the sweep

/// What one parameter set scores.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Scoreboard {
    mrr: f64,
    recall_at_1: f64,
    recall_at_3: f64,
    /// `symbol` queries only. The owner's decision of 2026-07-29 is that this
    /// is the category to optimise, because it is what an agent asks for over
    /// MCP, so the sweep treats it as a constraint rather than an average.
    symbol_mrr: f64,
    symbol_recall_at_1: f64,
    natural_mrr: f64,
    misspelling_mrr: f64,
}

fn evaluate(engine: &SearchEngine, queries: &[Query], ranking: &Ranking) -> Scoreboard {
    let mut all = Vec::with_capacity(queries.len());
    let mut symbol = Vec::new();
    let mut natural = Vec::new();
    let mut misspelling = Vec::new();

    for query in queries {
        let rank = rank_of(engine, query, ranking);
        all.push(rank);
        match query.kind.as_str() {
            "symbol" => symbol.push(rank),
            "natural" => natural.push(rank),
            "misspelling" => misspelling.push(rank),
            _ => {}
        }
    }

    let mrr = |ranks: &[usize]| {
        if ranks.is_empty() {
            return 0.0;
        }
        ranks
            .iter()
            .map(|r| if *r >= 1 { 1.0 / *r as f64 } else { 0.0 })
            .sum::<f64>()
            / ranks.len() as f64
    };
    let recall = |ranks: &[usize], k: usize| {
        if ranks.is_empty() {
            return 0.0;
        }
        ranks.iter().filter(|r| **r >= 1 && **r <= k).count() as f64 / ranks.len() as f64
    };

    Scoreboard {
        mrr: mrr(&all),
        recall_at_1: recall(&all, 1),
        recall_at_3: recall(&all, 3),
        symbol_mrr: mrr(&symbol),
        symbol_recall_at_1: recall(&symbol, 1),
        natural_mrr: mrr(&natural),
        misspelling_mrr: mrr(&misspelling),
    }
}

/// One line of the sweep table.
fn sweep_row(label: &str, ranking: &Ranking, board: &Scoreboard) -> String {
    format!(
        "  {label:<26} t={:<5.2} h={:<4.1} b={:<4.1} c={:<4.1} pivot={:<6} pen={:<4.2} sw={:<10} \
         fz={:<4.2}/{} | MRR {:.4}  r@1 {:.4}  r@3 {:.4}  sym {:.4}  nat {:.4}  ms {:.4}",
        ranking.title,
        ranking.headers,
        ranking.body,
        ranking.code,
        ranking.length_pivot,
        ranking.length_penalty,
        format!("{:?}", ranking.stopwords),
        ranking.fuzzy,
        ranking.fuzzy_max_distance,
        board.mrr,
        board.recall_at_1,
        board.recall_at_3,
        board.symbol_mrr,
        board.natural_mrr,
        board.misspelling_mrr,
    )
}

/// Coordinate descent over the ranking parameters.
///
/// `cargo test -p tome-core --test relevance -- --ignored --nocapture sweep`
///
/// **Ignored on purpose, and it is not a gate.** It optimises against the eval
/// set, so of course it improves on it; the number that means anything is what
/// `relevance_does_not_regress` reports afterwards on the committed baseline.
/// Running it as part of `check.sh` would also silently pin CI's wall clock to
/// a search of a few hundred configurations.
///
/// The output is a transcript, not a verdict: read which coordinate moved and
/// by how much, then set [`Ranking::TUNED`] by hand. Coordinate descent finds a
/// local optimum on 207 queries, which is not the same thing as a good ranker,
/// and a parameter that buys 0.002 MRR has been fitted to this corpus rather
/// than learned from it.
#[test]
#[ignore = "a tuning tool, not a gate: run it by hand when tuning ranking"]
fn sweep_ranking_parameters() {
    let dir = corpus_dir().join("relevance");
    let corpus: Corpus = serde_yaml_ng::from_str(
        &std::fs::read_to_string(dir.join("corpus.yaml")).expect("read corpus.yaml"),
    )
    .expect("parse corpus.yaml");
    let queries: QuerySet = serde_yaml_ng::from_str(
        &std::fs::read_to_string(dir.join("queries.yaml")).expect("read queries.yaml"),
    )
    .expect("parse queries.yaml");

    let documents = load_documents(&corpus.sources);
    let index_dir = tempfile::tempdir().expect("tempdir");
    let engine = build_index(index_dir.path(), &documents);
    let queries = queries.queries;

    println!(
        "\nranking sweep — {} queries, {} documents",
        queries.len(),
        documents.len()
    );

    let untuned = evaluate(&engine, &queries, &Ranking::UNTUNED);
    println!("{}", sweep_row("before S2-4", &Ranking::UNTUNED, &untuned));

    // The owner's constraint, made mechanical: a configuration that ranks
    // symbol lookups worse than the untuned ranker did is rejected outright,
    // however much it helps the average.
    let symbol_floor = untuned.symbol_recall_at_1;
    println!("  symbol recall@1 floor: {symbol_floor:.4}\n");

    // The two objectives worth descending on, run separately rather than
    // blended. MRR is the smoother signal; recall@3 is what the Stage 2 exit
    // gate actually asks for, and a weighted sum of the two would optimise
    // neither while looking principled.
    for (objective, pick) in [
        ("MRR", (|b: &Scoreboard| b.mrr) as fn(&Scoreboard) -> f64),
        ("recall@3", |b: &Scoreboard| b.recall_at_3),
    ] {
        println!("  ── descending on {objective} ──");
        let (best, board) = descend(&engine, &queries, symbol_floor, pick);
        println!(
            "{}",
            sweep_row(&format!("optimum ({objective})"), &best, &board)
        );
        println!();
    }

    println!("{}", sweep_row("before S2-4", &Ranking::UNTUNED, &untuned));
    let tuned = evaluate(&engine, &queries, &Ranking::TUNED);
    println!("{}", sweep_row("Ranking::TUNED", &Ranking::TUNED, &tuned));

    // Coordinate descent is greedy and path-dependent: it reports the first
    // local optimum it walks into, which is not necessarily near the shipped
    // configuration. This asks the question that actually matters — is
    // `TUNED` itself a local optimum, and what does each neighbour cost? —
    // and it answers it in full columns rather than one objective, because
    // the choice between two neighbours is a judgement about which category
    // to favour.
    println!("\n  ── one step from Ranking::TUNED ──");
    for (name, neighbour) in [
        (
            "title 0.5",
            Ranking {
                title: 0.5,
                ..Ranking::TUNED
            },
        ),
        (
            "title 1.0",
            Ranking {
                title: 1.0,
                ..Ranking::TUNED
            },
        ),
        (
            "title 1.5",
            Ranking {
                title: 1.5,
                ..Ranking::TUNED
            },
        ),
        (
            "headers 1.5",
            Ranking {
                headers: 1.5,
                ..Ranking::TUNED
            },
        ),
        (
            "headers 3.0",
            Ranking {
                headers: 3.0,
                ..Ranking::TUNED
            },
        ),
        (
            "code 0.5",
            Ranking {
                code: 0.5,
                ..Ranking::TUNED
            },
        ),
        (
            "code 1.5",
            Ranking {
                code: 1.5,
                ..Ranking::TUNED
            },
        ),
        (
            "pivot 1000",
            Ranking {
                length_pivot: 1_000,
                ..Ranking::TUNED
            },
        ),
        (
            "pivot 4000",
            Ranking {
                length_pivot: 4_000,
                ..Ranking::TUNED
            },
        ),
        (
            "penalty 0.2",
            Ranking {
                length_penalty: 0.2,
                ..Ranking::TUNED
            },
        ),
        (
            "penalty 0.6",
            Ranking {
                length_penalty: 0.6,
                ..Ranking::TUNED
            },
        ),
        (
            "stopwords Questions",
            Ranking {
                stopwords: StopwordPolicy::Questions,
                ..Ranking::TUNED
            },
        ),
        (
            "stopwords None",
            Ranking {
                stopwords: StopwordPolicy::None,
                ..Ranking::TUNED
            },
        ),
        (
            "fuzzy off",
            Ranking {
                fuzzy: 0.0,
                ..Ranking::TUNED
            },
        ),
        (
            "fuzzy 0.4",
            Ranking {
                fuzzy: 0.4,
                ..Ranking::TUNED
            },
        ),
        (
            "fuzzy 1.0",
            Ranking {
                fuzzy: 1.0,
                ..Ranking::TUNED
            },
        ),
        (
            "fuzzy distance 1",
            Ranking {
                fuzzy_max_distance: 1,
                ..Ranking::TUNED
            },
        ),
    ] {
        let board = evaluate(&engine, &queries, &neighbour);
        println!("{}", sweep_row(name, &neighbour, &board));
    }

    println!(
        "\n  If an optimum differs from `Ranking::TUNED`, that is a decision to make, \
         not a diff to apply."
    );
}

/// What typo tolerance costs, measured (P2-009 budgets 20 ms).
///
/// `cargo test -p tome-core --test relevance -- --ignored --nocapture cost`
///
/// Ignored for the same reason the sweep is: it is a measurement, and a
/// wall-clock assertion in the gate would fail on a loaded machine and be
/// suppressed within a week. **Run it in `--release`** — a debug build
/// measures rustc's bounds checks, not the search.
///
/// The number that matters is the *correctly spelled* row. Every term in such
/// a query is found in the term dictionary, so no scan happens at all, and
/// that is the query nearly every user issues nearly every time. The
/// misspelled row is what a typo costs on the rare occasion someone makes one.
#[test]
#[ignore = "a measurement, not a gate: wall-clock assertions fail on loaded machines"]
fn fuzzy_cost() {
    let dir = corpus_dir().join("relevance");
    let corpus: Corpus = serde_yaml_ng::from_str(
        &std::fs::read_to_string(dir.join("corpus.yaml")).expect("read corpus.yaml"),
    )
    .expect("parse corpus.yaml");
    let documents = load_documents(&corpus.sources);
    let index_dir = tempfile::tempdir().expect("tempdir");
    let engine = build_index(index_dir.path(), &documents);

    let off = Ranking {
        fuzzy: 0.0,
        ..Ranking::TUNED
    };

    println!("\nfuzzy cost — {} documents\n", documents.len());
    println!(
        "  {:<34} {:>12} {:>12} {:>10}",
        "query", "exact", "tolerant", "overhead"
    );

    for (label, query) in [
        ("correctly spelled", "environment variables"),
        ("correctly spelled (symbol)", "Vec::with_capacity"),
        ("one misspelling", "enviroment variables"),
        ("two misspellings", "enviroment varaibles"),
    ] {
        // Warm the caches for both paths before either is timed, or the first
        // measurement pays for the second's page faults.
        for _ in 0..20 {
            let _ = engine.search_with(query, 10, &off);
            let _ = engine.search_with(query, 10, &Ranking::TUNED);
        }

        let time = |ranking: &Ranking| {
            let start = std::time::Instant::now();
            for _ in 0..100 {
                engine.search_with(query, 10, ranking).expect("search");
            }
            start.elapsed() / 100
        };

        let exact = time(&off);
        let tolerant = time(&Ranking::TUNED);
        println!(
            "  {label:<34} {:>10.3?} {:>10.3?} {:>10.3?}",
            exact,
            tolerant,
            tolerant.saturating_sub(exact)
        );
    }
    println!(
        "\n  Corpus vocabulary, not page count, is what a correction scan reads — see \
         SPIKE-003 finding 2. At 100k pages this needs re-measuring; that is S2-12's."
    );
}

/// Coordinate descent from [`Ranking::UNTUNED`], maximising `pick`.
fn descend(
    engine: &SearchEngine,
    queries: &[Query],
    symbol_floor: f64,
    pick: fn(&Scoreboard) -> f64,
) -> (Ranking, Scoreboard) {
    // Each coordinate, and the values it may take. `body` stays at 1.0
    // throughout: the boosts are only meaningful relative to one another, so
    // fixing one removes a redundant dimension from the search.
    //
    // The length pivot and penalty are **one coordinate, not two**. They are
    // multiplicative — a pivot with a zero penalty does nothing, and a penalty
    // with a zero pivot does nothing — so coordinate descent varying either
    // alone from the untuned start finds both inert and never moves. The first
    // version of this sweep did exactly that and reported, quite confidently,
    // that length normalisation was worthless.
    const PIVOTS: [u32; 7] = [0, 250, 500, 1_000, 2_000, 4_000, 8_000];
    const PENALTIES: [f32; 6] = [0.0, 0.2, 0.4, 0.6, 1.0, 1.6];

    #[allow(clippy::type_complexity)]
    let coordinates: Vec<(&str, Box<dyn Fn(&mut Ranking, usize)>, usize)> = vec![
        (
            "title",
            Box::new(|r: &mut Ranking, i: usize| {
                r.title = [0.05, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0][i];
            }),
            9,
        ),
        (
            "headers",
            Box::new(|r: &mut Ranking, i: usize| {
                r.headers = [0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0][i];
            }),
            7,
        ),
        (
            "code",
            Box::new(|r: &mut Ranking, i: usize| {
                r.code = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0][i];
            }),
            6,
        ),
        (
            "length",
            Box::new(|r: &mut Ranking, i: usize| {
                r.length_pivot = PIVOTS[i / PENALTIES.len()];
                r.length_penalty = PENALTIES[i % PENALTIES.len()];
            }),
            PIVOTS.len() * PENALTIES.len(),
        ),
        (
            "stopwords",
            Box::new(|r: &mut Ranking, i: usize| {
                r.stopwords = [
                    StopwordPolicy::None,
                    StopwordPolicy::Questions,
                    StopwordPolicy::Function,
                ][i];
            }),
            3,
        ),
        (
            // Also a multiplicative pair: a boost with a zero distance cap has
            // nothing to weight, and a distance cap with a zero boost weights
            // it at nothing. Swept jointly for the same reason `length` is.
            "fuzzy",
            Box::new(|r: &mut Ranking, i: usize| {
                r.fuzzy = [0.0, 0.2, 0.4, 0.6, 0.8, 1.0, 1.5][i / 3];
                r.fuzzy_max_distance = [0, 1, 2][i % 3];
            }),
            21,
        ),
    ];

    let mut best = Ranking::UNTUNED;
    let mut best_board = evaluate(engine, queries, &best);

    // Several passes. The coordinates interact — a length penalty changes
    // which title boost is right — so one pass would only report the effect of
    // each parameter against the *untuned* value of every other.
    for pass in 1..=4 {
        let mut moved = false;
        for (name, set, count) in &coordinates {
            let mut local_best = best;
            let mut local_board = best_board;
            for i in 0..*count {
                let mut candidate = best;
                set(&mut candidate, i);
                if candidate == best {
                    continue;
                }
                let board = evaluate(engine, queries, &candidate);
                // The owner's constraint: symbol lookup is not traded away,
                // however much a candidate helps the average.
                if board.symbol_recall_at_1 + f64::EPSILON < symbol_floor {
                    continue;
                }
                if pick(&board) > pick(&local_board) {
                    local_best = candidate;
                    local_board = board;
                }
            }
            if local_best != best {
                println!(
                    "    pass {pass}: {name:<10} {:.4} → {:.4}",
                    pick(&best_board),
                    pick(&local_board)
                );
                best = local_best;
                best_board = local_board;
                moved = true;
            }
        }
        if !moved {
            println!("    pass {pass}: converged");
            break;
        }
    }
    (best, best_board)
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
