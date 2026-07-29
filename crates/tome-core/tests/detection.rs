//! Platform detection, scored against real homepages (S2-9, spec P2-020).
//!
//! ```bash
//! cargo test -p tome-core --test detection -- --nocapture   # see the matrix
//! TOME_UPDATE_DETECTION_BASELINE=1 cargo test -p tome-core --test detection
//! git diff -- crates/tome-core/corpus/detection/baseline.json
//! ```
//!
//! Update mode **fails the run it changes anything in**, the same way
//! `TOME_UPDATE_BASELINE` and `TOME_UPDATE_GOLDEN` do: the passing run is the
//! one after the diff has been read.
//!
//! # Why this exists before the detector does
//!
//! At this commit `detect` is a baseline that answers `Generic` at low
//! confidence for everything, and this harness duly scores it at 20 %. That is
//! the point. S2-1 established the order for search and every measured finding
//! in Stage 2 came out of it — the query-parser defect, the code-block symbol
//! defect, the all-declarations ranking regression — each invisible to
//! inspection and obvious to a corpus. S2-10 replaces the detector's body with
//! a number to beat and a confusion matrix to explain it.
//!
//! # What the corpus is, and what it is not
//!
//! 129 real homepages, fetched by `scripts/fetch-detection-corpus.mjs`, each
//! carrying its URL, capture date, licence and label. Everything is offline:
//! the fixtures are committed and no test touches the network.
//!
//! **The labels are ground truth asserted by a person, not derived from the
//! page**, and that distinction is load-bearing: a corpus labelled by the same
//! markers the detector reads would score the detector against itself. Where a
//! page self-identifies through `<meta name="generator">` the fetch script
//! cross-checks and reports disagreements; on the first run it caught eight,
//! of which two were sites that had genuinely migrated (pydantic to Astro,
//! swc.rs to Rspress) and six were Zensical, which emits Material-for-MkDocs
//! markup and is therefore still `mkdocs` for the only purpose the label has.
//!
//! **The label answers "which scraper handles this", not "which program
//! emitted it".** Those come apart, and when they do the scraper wins.
//!
//! **`GitBook` has no fixtures.** It is a hosted product now, and its public
//! instances are companies' own documentation under no redistributable
//! licence. The matrix reports an empty row rather than a score, because
//! nothing here measures it.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tome_core::detect::{detect, Evidence, Platform, AUTO_ACCEPT};

/// P2-020's floor. Asserted rather than assumed: a corpus that quietly shrank
/// would still produce a plausible-looking accuracy.
const MIN_FIXTURES: usize = 100;

/// How far accuracy may fall before the run fails.
///
/// Two fixtures of 129. Tighter than the relevance gate's margin because a
/// classification is discrete — there is no equivalent of BM25 reordering
/// ties, so a change that moves nothing should move *nothing*.
const MARGIN: f64 = 0.016;

/// One committed homepage.
struct Fixture {
    name: String,
    url: String,
    captured: String,
    licence: String,
    expected: Platform,
    headers: BTreeMap<String, String>,
    html: String,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
struct Baseline {
    accuracy: f64,
    /// How many were classified confidently *and* wrongly. P2-020's real
    /// success metric — a confident wrong answer sends a crawler at a site
    /// with the wrong scraper, where an unsure one asks.
    confident_errors: usize,
    /// Fixture name → what the detector said. A `BTreeMap` so the committed
    /// file has a stable order and its diff is readable.
    predictions: BTreeMap<String, String>,
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/detection")
}

/// Parse one `.fixture` file: `key: value` lines, `---`, then the HTML.
fn parse(name: &str, raw: &str) -> Fixture {
    let (front, html) = raw
        .split_once("\n---\n")
        .unwrap_or_else(|| panic!("{name}: no `---` separator"));

    let mut fields: BTreeMap<&str, String> = BTreeMap::new();
    let mut headers = BTreeMap::new();
    for line in front.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(": ") else {
            continue;
        };
        if key == "header" {
            if let Some((header, header_value)) = value.split_once(": ") {
                headers.insert(header.to_lowercase(), header_value.to_owned());
            }
        } else {
            fields.insert(key, value.to_owned());
        }
    }

    let field = |key: &str| {
        fields
            .get(key)
            .unwrap_or_else(|| panic!("{name}: missing `{key}`"))
            .clone()
    };
    let label = field("platform");

    Fixture {
        name: name.to_owned(),
        url: field("url"),
        captured: field("captured"),
        licence: field("licence"),
        expected: Platform::parse(&label)
            .unwrap_or_else(|| panic!("{name}: `{label}` is not a platform")),
        headers,
        html: html.to_owned(),
    }
}

fn load() -> Vec<Fixture> {
    let root = corpus_dir().join("fixtures");
    let mut fixtures = Vec::new();
    let mut stack = vec![root.clone()];

    while let Some(dir) = stack.pop() {
        let entries =
            std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "fixture") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("fixture name")
                .to_owned();
            let raw = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            fixtures.push(parse(&name, &raw));
        }
    }
    fixtures.sort_by(|a, b| a.name.cmp(&b.name));
    fixtures
}

#[test]
fn detection_does_not_regress() {
    let fixtures = load();

    assert!(
        fixtures.len() >= MIN_FIXTURES,
        "P2-020 requires at least {MIN_FIXTURES} fixtures, found {}",
        fixtures.len()
    );

    // Every fixture must carry its provenance. A fixture with no capture date
    // is a fixture whose staleness cannot be seen, which P2-020 asks for by
    // name; one with no licence is one nobody checked before publishing it.
    for fixture in &fixtures {
        assert!(!fixture.url.is_empty(), "{}: no url", fixture.name);
        assert!(
            fixture.captured.len() == 10 && fixture.captured.starts_with("20"),
            "{}: capture date {:?} is not a date",
            fixture.name,
            fixture.captured
        );
        assert!(!fixture.licence.is_empty(), "{}: no licence", fixture.name);
        assert!(
            fixture.html.len() > 200,
            "{}: {} bytes of HTML is not a page",
            fixture.name,
            fixture.html.len()
        );
    }

    // At least two labels must have enough fixtures to mean anything. A corpus
    // that was 95 % one class would score 95 % for a detector that always
    // guessed it — the trap S2-1 measured the hard way at 26 documents.
    let mut per_label: BTreeMap<Platform, usize> = BTreeMap::new();
    for fixture in &fixtures {
        *per_label.entry(fixture.expected).or_default() += 1;
    }
    let largest = per_label.values().copied().max().unwrap_or(0);
    assert!(
        (largest as f64) / (fixtures.len() as f64) < 0.5,
        "the largest class is {largest} of {} fixtures — a detector that always \
         guessed it would score well and know nothing",
        fixtures.len()
    );

    let mut predictions = BTreeMap::new();
    let mut correct = 0usize;
    let mut confident_errors = Vec::new();
    // [expected][predicted]
    let mut matrix: BTreeMap<Platform, BTreeMap<Platform, usize>> = BTreeMap::new();

    for fixture in &fixtures {
        let detection = detect(&Evidence {
            url: &fixture.url,
            headers: &fixture.headers,
            html: &fixture.html,
        });

        assert!(
            (0.0..=1.0).contains(&detection.confidence),
            "{}: confidence {} is out of range",
            fixture.name,
            detection.confidence
        );

        predictions.insert(fixture.name.clone(), detection.platform.as_str().to_owned());
        *matrix
            .entry(fixture.expected)
            .or_default()
            .entry(detection.platform)
            .or_default() += 1;

        if detection.platform == fixture.expected {
            correct += 1;
        } else if detection.is_confident() {
            confident_errors.push(format!(
                "    {} — expected {}, said {} at {:.2}",
                fixture.name,
                fixture.expected.as_str(),
                detection.platform.as_str(),
                detection.confidence
            ));
        }
    }

    let current = Baseline {
        accuracy: correct as f64 / fixtures.len() as f64,
        confident_errors: confident_errors.len(),
        predictions,
    };

    let report = report(&fixtures, &matrix, &current, &confident_errors);
    let baseline_path = corpus_dir().join("baseline.json");
    let updating = std::env::var_os("TOME_UPDATE_DETECTION_BASELINE").is_some();

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

    let moved = movement(&previous, &current);

    if updating {
        if previous == current {
            println!("{report}\nBaseline unchanged.");
            return;
        }
        write_baseline(&baseline_path, &current);
        panic!(
            "{moved}\n{report}\nBaseline rewritten. Review the diff, then re-run without \
             TOME_UPDATE_DETECTION_BASELINE."
        );
    }

    let mut failures = Vec::new();
    if previous.accuracy - current.accuracy > MARGIN {
        failures.push(format!(
            "accuracy: {:.4} → {:.4} (margin {MARGIN})",
            previous.accuracy, current.accuracy
        ));
    }
    if current.confident_errors > previous.confident_errors {
        // The metric P2-020 cares about most, gated with no margin at all: a
        // confident wrong answer is not noise, it is a crawl of the wrong
        // shape that nobody was asked about.
        failures.push(format!(
            "confidently wrong: {} → {}",
            previous.confident_errors, current.confident_errors
        ));
    }

    assert!(
        failures.is_empty(),
        "{report}\n{moved}\nDetection regressed:\n  {}\n\n\
         If this change is intended, re-run with TOME_UPDATE_DETECTION_BASELINE=1 and \
         commit the new baseline with the reason.",
        failures.join("\n  ")
    );

    println!("{report}\n{moved}");
}

fn write_baseline(path: &Path, baseline: &Baseline) {
    let json = serde_json::to_string_pretty(baseline).expect("serialize baseline");
    std::fs::write(path, format!("{json}\n")).expect("write baseline");
}

/// The confusion matrix P2-020 asks for by name.
fn report(
    fixtures: &[Fixture],
    matrix: &BTreeMap<Platform, BTreeMap<Platform, usize>>,
    current: &Baseline,
    confident_errors: &[String],
) -> String {
    let mut out = String::from("\ndetection\n─────────\n");
    let _ = writeln!(out, "  fixtures        {}", fixtures.len());
    let _ = writeln!(out, "  accuracy        {:.4}", current.accuracy);
    let _ = writeln!(
        out,
        "  confidently wrong  {}   (P2-020 wants 0)",
        current.confident_errors
    );
    let _ = writeln!(out, "  auto-accept at  {AUTO_ACCEPT:.2}");

    let _ = write!(out, "\n  actual \\ predicted ");
    for platform in Platform::ALL {
        let _ = write!(out, "{:>11}", platform.as_str());
    }
    let _ = writeln!(out, "{:>8}", "n");

    for expected in Platform::ALL {
        let row = matrix.get(&expected);
        let total: usize = row.map_or(0, |r| r.values().sum());
        if total == 0 {
            // An empty row is information: nothing in the corpus measures this
            // class. GitBook is deliberately one — see the module docs.
            let _ = writeln!(out, "  {:<18} {:>77}", expected.as_str(), "— no fixtures —");
            continue;
        }
        let _ = write!(out, "  {:<18} ", expected.as_str());
        for predicted in Platform::ALL {
            let count = row.and_then(|r| r.get(&predicted)).copied().unwrap_or(0);
            let _ = write!(
                out,
                "{:>11}",
                if count == 0 {
                    ".".to_owned()
                } else {
                    count.to_string()
                }
            );
        }
        let _ = writeln!(out, "{total:>8}");
    }

    if !confident_errors.is_empty() {
        let _ = writeln!(
            out,
            "\n  confidently wrong ({}) — each of these would crawl with the wrong scraper:",
            confident_errors.len()
        );
        for line in confident_errors {
            let _ = writeln!(out, "{line}");
        }
    }
    out
}

/// Which fixtures changed classification.
fn movement(previous: &Baseline, current: &Baseline) -> String {
    let mut changed = Vec::new();
    for (name, now) in &current.predictions {
        match previous.predictions.get(name) {
            None => changed.push(format!("    + {name}  → {now}")),
            Some(was) if was != now => changed.push(format!("    {name}  {was} → {now}")),
            Some(_) => {}
        }
    }
    for name in previous.predictions.keys() {
        if !current.predictions.contains_key(name) {
            changed.push(format!("    - {name}"));
        }
    }

    if changed.is_empty() {
        return "  no fixture changed classification".to_owned();
    }
    let mut out = format!("  changed ({}):\n", changed.len());
    for line in changed {
        let _ = writeln!(out, "{line}");
    }
    out
}
