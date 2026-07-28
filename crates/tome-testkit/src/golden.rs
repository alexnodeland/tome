//! The golden-corpus harness — implementation-plan **S0-7**.
//!
//! Some of Tome's output is *judged*, not asserted. Nobody can write
//! `assert_eq!` for "did this Sphinx page normalize well?", and a test that
//! only checks that normalization produced *some* HTML passes forever while
//! the output quietly degrades. The answer is to commit the expected output,
//! diff against it, and review the diff — which is the whole point: this is
//! the machinery that makes normalization quality reviewable at all, and it is
//! why S0-7 lands before any ingestion code.
//!
//! # Layout
//!
//! ```text
//! corpus/<suite>/
//! ├── input/                 one file per case, committed
//! │   ├── sphinx-basic.html
//! │   └── rustdoc-struct.html
//! └── golden/                the expected output, committed
//!     ├── sphinx-basic.html
//!     └── rustdoc-struct.html
//! ```
//!
//! # Use
//!
//! ```no_run
//! use tome_testkit::Golden;
//!
//! # fn normalize(html: &str) -> String { html.to_string() }
//! let report = Golden::new("corpus/normalization")
//!     .check(|case| normalize(&case.text()))
//!     .expect("read corpus");
//!
//! assert!(report.is_ok(), "{report}");
//! ```
//!
//! # Updating
//!
//! ```text
//! TOME_UPDATE_GOLDEN=1 cargo test -p tome-core normalization
//! git diff -- crates/tome-core/corpus     # <- the actual review step
//! ```
//!
//! Update mode **still fails the test** when it changes anything. A harness
//! that rewrites the goldens and reports success is a harness that lets a
//! regression be laundered into the expected output by a single command; the
//! second run — the one that passes — is the one that means the diff was
//! looked at.
//!
//! # What it checks beyond the diff
//!
//! - **An empty suite fails.** A golden suite with no cases passes vacuously
//!   forever, which is worse than having no suite at all.
//! - **Orphan goldens fail.** A golden with no matching input means an input
//!   was deleted or renamed and the expectation was left behind. These are
//!   *not* auto-removed even in update mode — deleting committed expectations
//!   is a decision, not a side effect.

use std::fmt;
use std::path::{Path, PathBuf};

/// Set to any non-empty value to rewrite goldens from actual output.
pub const UPDATE_ENV: &str = "TOME_UPDATE_GOLDEN";

/// Lines of diff shown per failing case before truncating. The full output is
/// always written to the `.actual` file next to the golden.
const MAX_DIFF_LINES: usize = 120;

/// One corpus case: an input file and the golden it is expected to produce.
#[derive(Debug, Clone)]
pub struct Case {
    /// File stem of the input, e.g. `sphinx-basic`.
    pub name: String,
    /// Path to the committed input.
    pub input_path: PathBuf,
    /// Path to the committed expected output. May not exist yet.
    pub golden_path: PathBuf,
    bytes: Vec<u8>,
}

impl Case {
    /// The input as text, lossily decoded.
    ///
    /// Lossy is right here: the corpus contains real pages, and a real page
    /// with a broken byte in it is a case worth keeping rather than a reason
    /// for the harness to fail.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }

    /// The input's raw bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// What happened to one case.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// Output matched the golden.
    Passed,
    /// Output differed. `diff` is unified, golden-then-actual.
    Mismatch { diff: String, actual_path: PathBuf },
    /// No golden committed for this input.
    MissingGolden,
    /// Update mode wrote or rewrote the golden.
    Updated,
    /// A golden with no corresponding input.
    OrphanGolden { golden_path: PathBuf },
}

impl Outcome {
    fn is_ok(&self) -> bool {
        matches!(self, Self::Passed)
    }
}

/// The result of running a suite. Not self-asserting on purpose: tests do
/// `assert!(report.is_ok(), "{report}")`, which keeps the panic in the test
/// file where the workspace lints allow it.
#[derive(Debug, Clone)]
pub struct Report {
    suite: String,
    updating: bool,
    entries: Vec<(String, Outcome)>,
}

impl Report {
    /// Whether every case passed. An empty suite is *not* ok.
    pub fn is_ok(&self) -> bool {
        !self.entries.is_empty() && self.entries.iter().all(|(_, o)| o.is_ok())
    }

    /// Number of cases that matched their golden.
    pub fn passed(&self) -> usize {
        self.entries.iter().filter(|(_, o)| o.is_ok()).count()
    }

    /// Number of cases that did not.
    pub fn failed(&self) -> usize {
        self.entries.len() - self.passed()
    }

    /// Every outcome, in case order.
    pub fn outcomes(&self) -> &[(String, Outcome)] {
        &self.entries
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.entries.is_empty() {
            return write!(
                f,
                "golden suite `{}` has no cases.\n\
                 An empty suite passes vacuously, so this is a failure: add inputs under \
                 `input/`, or delete the suite.",
                self.suite
            );
        }

        writeln!(
            f,
            "golden suite `{}`: {} passed, {} failed",
            self.suite,
            self.passed(),
            self.failed()
        )?;

        for (name, outcome) in &self.entries {
            match outcome {
                Outcome::Passed => {}
                Outcome::Mismatch { diff, actual_path } => {
                    writeln!(f, "\n── {name}: output differs from the golden ──")?;
                    let lines: Vec<&str> = diff.lines().collect();
                    for line in lines.iter().take(MAX_DIFF_LINES) {
                        writeln!(f, "{line}")?;
                    }
                    if lines.len() > MAX_DIFF_LINES {
                        writeln!(f, "… {} more diff lines", lines.len() - MAX_DIFF_LINES)?;
                    }
                    writeln!(f, "actual output: {}", actual_path.display())?;
                }
                Outcome::MissingGolden => {
                    writeln!(
                        f,
                        "\n── {name}: no golden committed. Review the output, then \
                         `{UPDATE_ENV}=1 cargo test` to record it."
                    )?;
                }
                Outcome::Updated => {
                    writeln!(f, "\n── {name}: golden updated")?;
                }
                Outcome::OrphanGolden { golden_path } => {
                    writeln!(
                        f,
                        "\n── {name}: golden with no input ({}). \
                         Delete it, or restore the input it belonged to.",
                        golden_path.display()
                    )?;
                }
            }
        }

        if self.updating && self.failed() > 0 {
            writeln!(
                f,
                "\n{UPDATE_ENV} is set: goldens were rewritten. Review `git diff` over the \
                 corpus, then re-run without it — that second run is the one that passes."
            )?;
        }

        Ok(())
    }
}

/// A golden corpus rooted at one directory. See the [module docs](self).
pub struct Golden {
    dir: PathBuf,
    golden_extension: Option<String>,
    updating: bool,
}

impl Golden {
    /// Open the suite rooted at `dir`, which must contain `input/`.
    ///
    /// Relative paths resolve against the current directory, which for
    /// `cargo test` is the crate root — so `Golden::new("corpus/normalization")`
    /// in `tome-core` means `crates/tome-core/corpus/normalization`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            golden_extension: None,
            updating: std::env::var_os(UPDATE_ENV).is_some_and(|v| !v.is_empty()),
        }
    }

    /// Extension for golden files, when the transform changes format —
    /// `.html` in, `.json` out. Defaults to the input's own extension.
    pub fn golden_extension(mut self, extension: &str) -> Self {
        self.golden_extension = Some(extension.trim_start_matches('.').to_string());
        self
    }

    /// Force update mode on or off, ignoring the environment. For testing the
    /// harness itself.
    pub fn updating(mut self, updating: bool) -> Self {
        self.updating = updating;
        self
    }

    /// Every case in the suite, sorted by name.
    pub fn cases(&self) -> std::io::Result<Vec<Case>> {
        let input_dir = self.dir.join("input");
        let mut cases = Vec::new();

        let listing = std::fs::read_dir(&input_dir).map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!("golden corpus {} is not readable: {e}", input_dir.display()),
            )
        })?;

        for entry in listing {
            let path = entry?.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            // Editor droppings, per-directory READMEs, and the SOURCES.md
            // provenance file (the corpus convention places it in input/) are
            // documentation, not cases.
            if name.starts_with('.')
                || name.eq_ignore_ascii_case("readme")
                || name.eq_ignore_ascii_case("sources")
            {
                continue;
            }

            let extension = self
                .golden_extension
                .clone()
                .or_else(|| path.extension().and_then(|e| e.to_str()).map(String::from))
                .unwrap_or_else(|| "txt".to_string());

            cases.push(Case {
                name: name.to_string(),
                golden_path: self.dir.join("golden").join(format!("{name}.{extension}")),
                bytes: std::fs::read(&path)?,
                input_path: path,
            });
        }

        cases.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(cases)
    }

    /// Run `transform` over every case and compare with the goldens.
    ///
    /// Errors only for I/O problems with the corpus itself; a failing *case* is
    /// reported in the [`Report`], not returned as an error.
    pub fn check<F>(&self, transform: F) -> std::io::Result<Report>
    where
        F: Fn(&Case) -> String,
    {
        let cases = self.cases()?;
        let golden_dir = self.dir.join("golden");
        std::fs::create_dir_all(&golden_dir)?;

        let mut entries = Vec::new();

        for case in &cases {
            let actual = normalize_trailing_newline(&transform(case));
            let actual_path = with_suffix(&case.golden_path, ".actual");

            let expected = match std::fs::read_to_string(&case.golden_path) {
                Ok(text) => Some(normalize_trailing_newline(&text)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e),
            };

            let outcome = match expected {
                Some(expected) if expected == actual => {
                    // A stale `.actual` from an earlier failing run would go on
                    // misleading whoever opens the directory next.
                    let _ = std::fs::remove_file(&actual_path);
                    Outcome::Passed
                }
                Some(expected) => {
                    if self.updating {
                        std::fs::write(&case.golden_path, &actual)?;
                        let _ = std::fs::remove_file(&actual_path);
                        Outcome::Updated
                    } else {
                        std::fs::write(&actual_path, &actual)?;
                        Outcome::Mismatch {
                            diff: unified_diff(&expected, &actual),
                            actual_path,
                        }
                    }
                }
                None => {
                    if self.updating {
                        std::fs::write(&case.golden_path, &actual)?;
                        Outcome::Updated
                    } else {
                        std::fs::write(&actual_path, &actual)?;
                        Outcome::MissingGolden
                    }
                }
            };

            entries.push((case.name.clone(), outcome));
        }

        entries.extend(self.orphan_goldens(&cases, &golden_dir)?);

        Ok(Report {
            suite: self
                .dir
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("golden")
                .to_string(),
            updating: self.updating,
            entries,
        })
    }

    fn orphan_goldens(
        &self,
        cases: &[Case],
        golden_dir: &Path,
    ) -> std::io::Result<Vec<(String, Outcome)>> {
        let known: Vec<&Path> = cases.iter().map(|c| c.golden_path.as_path()).collect();
        let mut orphans = Vec::new();

        for entry in std::fs::read_dir(golden_dir)? {
            let path = entry?.path();
            if !path.is_file() || known.contains(&path.as_path()) {
                continue;
            }
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            // `.actual` files are this harness's own scratch output, not corpus.
            if name.starts_with('.') || name.ends_with(".actual") {
                continue;
            }

            orphans.push((
                name.to_string(),
                Outcome::OrphanGolden {
                    golden_path: path.clone(),
                },
            ));
        }

        orphans.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(orphans)
    }
}

/// Unified diff, golden first, actual second.
fn unified_diff(expected: &str, actual: &str) -> String {
    similar::TextDiff::from_lines(expected, actual)
        .unified_diff()
        .context_radius(3)
        .header("golden", "actual")
        .to_string()
}

/// Exactly one trailing newline, on both sides of every comparison.
///
/// Otherwise every editor that adds a final newline, and every transform that
/// does not, produces a diff about nothing.
fn normalize_trailing_newline(text: &str) -> String {
    format!("{}\n", text.trim_end_matches('\n'))
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
