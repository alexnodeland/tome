//! SPIKE-003 — Tantivy at documentation scale.
//!
//! The question (`docs/plans/07-technical-spikes.md` § SPIKE-003): what does
//! Tantivy cost, in memory and latency, for a library of 100 000 pages? Four
//! numbers gate Stage 2:
//!
//! | Criterion | Budget |
//! |---|---|
//! | Peak RSS while indexing 100k pages | < 1 GB |
//! | Search latency at 100k pages | < 100 ms |
//! | Index size on disk | < 500 MB |
//! | Idle memory, index not loaded | < 50 MB |
//!
//! # How memory is measured
//!
//! **Not from inside the process.** Each phase is a separate subcommand, run
//! under `/usr/bin/time -l`, which reports "maximum resident set size" from
//! the kernel. Reading `getrusage` in-process would need `unsafe`, and asking
//! a program to report its own peak memory is exactly the measurement most
//! likely to be quietly wrong. `run.sh` drives it.
//!
//! # The corpus is synthetic, and says so
//!
//! 100 000 real documentation pages is ~600 MB of HTML and cannot be
//! committed or fetched politely. The generator produces
//! documentation-*shaped* text — a title, headings, prose over a small
//! vocabulary, and code blocks — with a long-tailed size distribution around
//! a 6 KB body, which is what the corpus of real pages in
//! `crates/tome-core/corpus` actually averages.
//!
//! What synthetic text gets wrong is **term diversity**: a small vocabulary
//! makes the inverted index smaller and more compressible than reality. The
//! write-up states that as the main caveat on the index-size number; the
//! memory and latency numbers are far less sensitive to it.

use std::path::{Path, PathBuf};
use std::time::Instant;

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Schema, TextFieldIndexing, TextOptions, INDEXED, STORED, STRING, TEXT};
use tantivy::{doc, Index, IndexWriter, ReloadPolicy, TantivyDocument};

fn main() -> tantivy::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(String::as_str).unwrap_or("help");

    match command {
        "index" => {
            let dir = arg_path(&args, "--dir");
            let pages = arg_usize(&args, "--pages", 100_000);
            let budget_mb = arg_usize(&args, "--budget-mb", 128);
            let start_at = arg_usize(&args, "--start-at", 0);
            let vocab = arg_usize(&args, "--vocab", 0);
            index(&dir, pages, budget_mb, start_at, vocab)
        }
        "search" => {
            let dir = arg_path(&args, "--dir");
            let rounds = arg_usize(&args, "--rounds", 200);
            search(&dir, rounds)
        }
        "open" => {
            // Open the index and stop. The RSS of this run is the floor a
            // process pays merely for having a searchable library.
            let dir = arg_path(&args, "--dir");
            let index = Index::open_in_dir(&dir)?;
            println!("opened index with {} segments", index.searchable_segments()?.len());
            Ok(())
        }
        "idle" => {
            // Neither opens nor reads the index: the floor a process pays for
            // linking Tantivy at all. The difference between this and `open`
            // is what "index not loaded" costs.
            println!("linked tantivy, opened nothing");
            Ok(())
        }
        _ => {
            eprintln!(
                "usage:\n  \
                 index  --dir D [--pages N] [--budget-mb M] [--start-at N]\n  \
                 search --dir D [--rounds N]\n  \
                 open   --dir D\n  \
                 idle"
            );
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Schema — the one P2-002 specifies, so the numbers describe what will ship.
// ---------------------------------------------------------------------------

struct Fields {
    source_id: tantivy::schema::Field,
    path: tantivy::schema::Field,
    title: tantivy::schema::Field,
    headers: tantivy::schema::Field,
    body: tantivy::schema::Field,
    code: tantivy::schema::Field,
    category: tantivy::schema::Field,
}

fn build_schema() -> (Schema, Fields) {
    let mut builder = Schema::builder();

    // `code` gets its own tokenizer slot in P2-002 (camelCase/snake_case
    // aware). The tokenizer itself is S2-2's work; what matters here is that
    // the field exists and is indexed, because a field's *presence* is what
    // drives index size and memory, not how clever its tokenizer is.
    let code_options = TextOptions::default()
        .set_indexing_options(TextFieldIndexing::default().set_tokenizer("default"));

    let fields = Fields {
        source_id: builder.add_text_field("source_id", STRING | STORED),
        path: builder.add_text_field("path", STRING | STORED),
        title: builder.add_text_field("title", TEXT | STORED),
        headers: builder.add_text_field("headers", TEXT),
        body: builder.add_text_field("body", TEXT),
        code: builder.add_text_field("code", code_options),
        category: builder.add_facet_field("category", INDEXED),
    };
    (builder.build(), fields)
}

// ---------------------------------------------------------------------------
// Indexing
// ---------------------------------------------------------------------------

fn index(
    dir: &Path,
    pages: usize,
    budget_mb: usize,
    start_at: usize,
    vocab: usize,
) -> tantivy::Result<()> {
    std::fs::create_dir_all(dir)?;
    let (schema, fields) = build_schema();

    // Reopen when adding to an existing index (the incremental measurement),
    // create otherwise.
    let index = match Index::open_in_dir(dir) {
        Ok(existing) => existing,
        Err(_) => Index::create_in_dir(dir, schema)?,
    };

    // **The knob that decides peak RSS.** Tantivy buffers documents in memory
    // and flushes a segment when the budget is reached, so this is close to a
    // direct setting for how much the indexer costs — the point of measuring
    // several values rather than accepting a default.
    let mut writer: IndexWriter = index.writer(budget_mb * 1024 * 1024)?;

    let started = Instant::now();
    let mut bytes = 0u64;
    for n in start_at..start_at + pages {
        let page = generate_page(n as u64, vocab);
        bytes += page.body.len() as u64 + page.code.len() as u64;
        writer.add_document(doc!(
            fields.source_id => page.source_id,
            fields.path      => page.path,
            fields.title     => page.title,
            fields.headers   => page.headers,
            fields.body      => page.body,
            fields.code      => page.code,
            fields.category  => tantivy::schema::Facet::from(page.category.as_str()),
        ))?;

        if (n - start_at + 1) % 10_000 == 0 {
            println!(
                "  {} pages, {:.1}s elapsed",
                n - start_at + 1,
                started.elapsed().as_secs_f64()
            );
        }
    }

    let added = started.elapsed();
    println!("added {pages} pages in {:.1}s", added.as_secs_f64());

    let commit_started = Instant::now();
    writer.commit()?;
    println!("commit took {:.1}s", commit_started.elapsed().as_secs_f64());

    println!(
        "RESULT index pages={pages} budget_mb={budget_mb} \
         vocab={vocab} text_mb={:.0} total_s={:.1} pages_per_s={:.0}",
        bytes as f64 / 1_048_576.0,
        started.elapsed().as_secs_f64(),
        pages as f64 / started.elapsed().as_secs_f64()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

fn search(dir: &Path, rounds: usize) -> tantivy::Result<()> {
    let index = Index::open_in_dir(dir)?;
    let schema = index.schema();
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()?;

    let field = |name: &str| schema.get_field(name).expect("schema field");
    let parser = QueryParser::for_index(
        &index,
        vec![field("title"), field("headers"), field("body"), field("code")],
    );

    // A spread of query shapes, because they do not cost the same: a single
    // common term scans a long posting list, a rare term almost none, and a
    // phrase has to check positions.
    //
    // Every one of these must actually MATCH. A query with no hits measures
    // an empty posting list, which is trivially fast and says nothing — the
    // first run of this spike reported 0.01 ms for two queries that simply
    // found nothing.
    let queries: Vec<(&str, &str)> = vec![
        ("single common term", "configuration"),
        ("single rare term", "quixotic"),
        ("two terms", "configuration options"),
        ("phrase", "\"configuration options\""),
        ("code-only identifier", "config"),
        ("four terms", "configuration options default value"),
        ("prefix", "config*"),
    ];

    let searcher = reader.searcher();
    println!("segments={} docs={}", searcher.segment_readers().len(), searcher.num_docs());

    for (label, query_text) in &queries {
        let query = match parser.parse_query(query_text) {
            Ok(q) => q,
            Err(e) => {
                println!("RESULT search label={label:?} PARSE_ERROR {e}");
                continue;
            }
        };

        // Warm once so the first run's page-cache faults are not reported as
        // typical latency; both numbers are interesting, so cold is reported
        // separately.
        let cold = Instant::now();
        let _ = searcher.search(&query, &TopDocs::with_limit(20).order_by_score())?;
        let cold_ms = cold.elapsed().as_secs_f64() * 1000.0;

        let mut timings = Vec::with_capacity(rounds);
        let mut hits = 0;
        for _ in 0..rounds {
            let at = Instant::now();
            let top = searcher.search(&query, &TopDocs::with_limit(20).order_by_score())?;
            timings.push(at.elapsed().as_secs_f64() * 1000.0);
            hits = top.len();
        }
        timings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        println!(
            "RESULT search label={label:?} hits={hits} cold_ms={cold_ms:.2} \
             p50_ms={:.2} p95_ms={:.2} max_ms={:.2}",
            percentile(&timings, 0.50),
            percentile(&timings, 0.95),
            timings.last().copied().unwrap_or(0.0)
        );
    }

    // Retrieving stored fields is part of showing a result, so measure it.
    let query = parser.parse_query("configuration").expect("valid query");
    let top = searcher.search(&query, &TopDocs::with_limit(20).order_by_score())?;
    let at = Instant::now();
    for (_score, address) in &top {
        let _doc: TantivyDocument = searcher.doc(*address)?;
    }
    println!(
        "RESULT fetch_stored docs={} total_ms={:.2}",
        top.len(),
        at.elapsed().as_secs_f64() * 1000.0
    );

    Ok(())
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

// ---------------------------------------------------------------------------
// Synthetic corpus
// ---------------------------------------------------------------------------

struct Page {
    source_id: String,
    path: String,
    title: String,
    headers: String,
    body: String,
    code: String,
    category: String,
}

/// Deterministic, seeded, dependency-free. A spike whose corpus changes
/// between runs cannot be re-run to check a fix.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*: good enough for text generation, and one line.
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// A small vocabulary, on purpose — see the module docs on term diversity.
const WORDS: &[&str] = &[
    "configuration", "options", "default", "value", "returns", "function", "module", "package",
    "instance", "reference", "parameter", "argument", "example", "usage", "error", "exception",
    "handler", "request", "response", "buffer", "stream", "iterator", "collection", "sequence",
    "mapping", "identifier", "attribute", "property", "method", "constructor", "interface",
    "implementation", "behaviour", "specified", "optional", "required", "deprecated", "version",
    "release", "compatibility", "platform", "directory", "filesystem", "encoding", "unicode",
    "serialize", "deserialize", "validate", "resolve", "allocate", "concurrent", "asynchronous",
];

const CATEGORIES: &[&str] = &["/rust", "/python", "/javascript", "/go", "/systems"];

/// `vocab` is the size of the **long-tail** term pool. Zero keeps the small
/// fixed vocabulary; a non-zero value mixes in rare terms so the inverted
/// index has something like a realistic number of distinct terms.
///
/// This exists because index size is the one measurement a synthetic corpus
/// can flatter: 52 words compress far better than English plus identifiers.
/// The tail here is drawn UNIFORMLY, which is worse than the Zipfian
/// distribution real text has — so the number it produces is an upper bound,
/// which is the useful direction for a caveat.
fn generate_page(n: u64, vocab: usize) -> Page {
    let mut rng = Rng(n.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);

    let source = n % 50; // 50 sources, per the "power user" case in the spec.
    let category = CATEGORIES[(source as usize) % CATEGORIES.len()];

    // Long-tailed sizes: most pages modest, a few very large. Real
    // documentation looks like this — a tutorial page beside a 200 KB API
    // reference — and an even distribution would understate the peak.
    let scale = match rng.below(100) {
        0..=4 => 8.0,   // 5% very large
        5..=24 => 2.5,  // 20% large
        _ => 1.0,
    };
    let paragraphs = ((6.0 * scale) as usize).max(3);

    let mut body = String::with_capacity(paragraphs * 400);
    let mut headers = String::new();

    // A genuinely rare term, on ~1 page in 1000. Without one, the
    // "rare term" query has nothing to find and measures nothing; with it,
    // the query does the work a real long-tail lookup does.
    if n % 1000 == 0 {
        body.push_str("quixotic ");
    }
    for p in 0..paragraphs {
        if p % 3 == 0 {
            let heading = format!(
                "{} {}",
                WORDS[rng.below(WORDS.len())],
                WORDS[rng.below(WORDS.len())]
            );
            headers.push_str(&heading);
            headers.push(' ');
            body.push_str(&heading);
            body.push('\n');
        }
        let sentences = 3 + rng.below(4);
        for _ in 0..sentences {
            let words = 8 + rng.below(14);
            for _ in 0..words {
                // 70/30 common-to-tail, roughly what a documentation page
                // looks like once identifiers and proper nouns are counted.
                if vocab > 0 && rng.below(10) >= 7 {
                    body.push_str(&format!("t{}", rng.below(vocab)));
                } else {
                    body.push_str(WORDS[rng.below(WORDS.len())]);
                }
                body.push(' ');
            }
            body.push_str(". ");
        }
        body.push('\n');
    }

    let mut code = String::new();
    for _ in 0..(1 + rng.below(4)) {
        code.push_str(&format!(
            "fn {}_{}(config: &{}) -> Result<{}, Error> {{\n    config.{}()\n}}\n",
            WORDS[rng.below(WORDS.len())],
            rng.below(1000),
            WORDS[rng.below(WORDS.len())],
            WORDS[rng.below(WORDS.len())],
            WORDS[rng.below(WORDS.len())],
        ));
    }

    Page {
        source_id: format!("source-{source:02}"),
        path: format!("docs/{}/page-{n}.html", rng.below(20)),
        title: format!(
            "{} {} {}",
            WORDS[rng.below(WORDS.len())],
            WORDS[rng.below(WORDS.len())],
            n
        ),
        headers,
        body,
        code,
        category: category.to_owned(),
    }
}

// ---------------------------------------------------------------------------

fn arg_path(args: &[String], name: &str) -> PathBuf {
    PathBuf::from(arg_str(args, name).unwrap_or_else(|| {
        eprintln!("{name} is required");
        std::process::exit(2);
    }))
}

fn arg_str(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn arg_usize(args: &[String], name: &str, default: usize) -> usize {
    arg_str(args, name)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
