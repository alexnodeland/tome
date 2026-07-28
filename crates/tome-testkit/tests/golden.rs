//! Tests for the golden-corpus harness (S0-7).
//!
//! The harness is the thing that decides whether a normalization change was an
//! improvement or a regression, so its own failure modes have to be exact: a
//! suite that silently passes when it tests nothing, or an update mode that
//! reports success after rewriting the expectations, would both be worse than
//! having no harness.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::Path;

use tempfile::TempDir;
use tome_testkit::{Golden, Outcome};

/// Build a corpus directory: `(name, input)` pairs and `(name, golden)` pairs.
fn corpus(inputs: &[(&str, &str)], goldens: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("input")).expect("input dir");
    std::fs::create_dir_all(dir.path().join("golden")).expect("golden dir");

    for (name, body) in inputs {
        std::fs::write(dir.path().join("input").join(name), body).expect("write input");
    }
    for (name, body) in goldens {
        std::fs::write(dir.path().join("golden").join(name), body).expect("write golden");
    }
    dir
}

/// A stand-in for a normalization pass.
fn shout(case: &tome_testkit::golden::Case) -> String {
    case.text().to_uppercase()
}

#[test]
fn matching_output_passes() {
    let dir = corpus(
        &[("one.html", "<p>hello</p>"), ("two.html", "<p>world</p>")],
        &[
            ("one.html", "<P>HELLO</P>\n"),
            ("two.html", "<P>WORLD</P>\n"),
        ],
    );

    let report = Golden::new(dir.path()).check(shout).expect("check");

    assert!(report.is_ok(), "{report}");
    assert_eq!(report.passed(), 2);
    assert_eq!(report.failed(), 0);
}

#[test]
fn a_trailing_newline_is_not_a_diff() {
    // Editors add one, transforms usually do not. Without normalisation every
    // golden file would churn depending on who last opened it.
    let dir = corpus(&[("one.txt", "a")], &[("one.txt", "A\n\n\n")]);

    let report = Golden::new(dir.path()).check(shout).expect("check");

    assert!(report.is_ok(), "{report}");
}

#[test]
fn a_mismatch_reports_a_diff_and_writes_the_actual_output() {
    let dir = corpus(
        &[("page.html", "<p>hello</p>")],
        &[("page.html", "<P>GOODBYE</P>\n")],
    );

    let report = Golden::new(dir.path()).check(shout).expect("check");

    assert!(!report.is_ok());
    assert_eq!(report.failed(), 1);

    let Some((name, Outcome::Mismatch { diff, actual_path })) = report.outcomes().first() else {
        panic!("expected a mismatch, got {:?}", report.outcomes());
    };
    assert_eq!(name, "page");
    assert!(diff.contains("-<P>GOODBYE</P>"), "diff was:\n{diff}");
    assert!(diff.contains("+<P>HELLO</P>"), "diff was:\n{diff}");

    // The full output goes next to the golden so it can be inspected or copied
    // over deliberately, rather than only existing inside a test failure.
    assert_eq!(
        std::fs::read_to_string(actual_path).expect("actual file"),
        "<P>HELLO</P>\n"
    );

    // The rendered report is what a developer actually reads.
    let rendered = report.to_string();
    assert!(rendered.contains("page"), "{rendered}");
    assert!(rendered.contains("1 failed"), "{rendered}");
}

#[test]
fn a_passing_case_clears_a_stale_actual_file() {
    let dir = corpus(
        &[("page.html", "<p>hi</p>")],
        &[("page.html", "<P>NOPE</P>\n")],
    );
    let actual = dir.path().join("golden/page.html.actual");

    assert!(!Golden::new(dir.path()).check(shout).expect("check").is_ok());
    assert!(actual.exists());

    std::fs::write(dir.path().join("golden/page.html"), "<P>HI</P>\n").expect("fix golden");
    assert!(Golden::new(dir.path()).check(shout).expect("check").is_ok());
    assert!(
        !actual.exists(),
        "a stale .actual left behind misleads whoever opens the directory next"
    );
}

#[test]
fn a_missing_golden_is_a_failure_not_an_auto_accept() {
    let dir = corpus(&[("page.html", "<p>new case</p>")], &[]);

    let report = Golden::new(dir.path()).check(shout).expect("check");

    assert!(!report.is_ok());
    assert!(matches!(
        report.outcomes().first(),
        Some((_, Outcome::MissingGolden))
    ));
    assert!(report.to_string().contains("TOME_UPDATE_GOLDEN"));
}

#[test]
fn update_mode_writes_goldens_and_still_fails_the_run() {
    let dir = corpus(
        &[("page.html", "<p>hello</p>")],
        &[("page.html", "<P>STALE</P>\n")],
    );

    let updated = Golden::new(dir.path())
        .updating(true)
        .check(shout)
        .expect("check");

    assert!(
        !updated.is_ok(),
        "update mode must fail the run it changes anything in — otherwise a \
         regression can be laundered into the expected output by one command"
    );
    assert!(matches!(
        updated.outcomes().first(),
        Some((_, Outcome::Updated))
    ));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("golden/page.html")).expect("golden"),
        "<P>HELLO</P>\n"
    );

    // The second run — after the diff has been reviewed — is the passing one.
    let confirmed = Golden::new(dir.path()).check(shout).expect("check");
    assert!(confirmed.is_ok(), "{confirmed}");
}

#[test]
fn a_golden_without_an_input_is_reported() {
    let dir = corpus(
        &[("kept.html", "<p>kept</p>")],
        &[
            ("kept.html", "<P>KEPT</P>\n"),
            ("removed.html", "<P>GONE</P>\n"),
        ],
    );

    let report = Golden::new(dir.path()).check(shout).expect("check");

    assert!(!report.is_ok());
    let orphans: Vec<_> = report
        .outcomes()
        .iter()
        .filter(|(_, o)| matches!(o, Outcome::OrphanGolden { .. }))
        .collect();
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0].0, "removed.html");

    // Even in update mode: deleting a committed expectation is a decision.
    let updated = Golden::new(dir.path())
        .updating(true)
        .check(shout)
        .expect("check");
    assert!(!updated.is_ok());
    assert!(dir.path().join("golden/removed.html").exists());
}

#[test]
fn an_empty_suite_fails() {
    let dir = corpus(&[], &[]);

    let report = Golden::new(dir.path()).check(shout).expect("check");

    assert!(
        !report.is_ok(),
        "a suite with no cases passes vacuously forever; that is the failure \
         mode a golden harness exists to prevent"
    );
    assert!(report.to_string().contains("no cases"), "{report}");
}

#[test]
fn readmes_and_dotfiles_are_not_cases() {
    let dir = corpus(
        &[
            ("README.md", "how this corpus works"),
            ("page.html", "<p>x</p>"),
        ],
        &[("page.html", "<P>X</P>\n")],
    );
    std::fs::write(dir.path().join("input/.DS_Store"), "junk").expect("write");

    let report = Golden::new(dir.path()).check(shout).expect("check");

    assert!(report.is_ok(), "{report}");
    assert_eq!(report.passed(), 1);
}

#[test]
fn the_golden_extension_can_differ_from_the_input() {
    let dir = corpus(&[("page.html", "<p>x</p>")], &[]);

    let report = Golden::new(dir.path())
        .golden_extension("json")
        .updating(true)
        .check(|case| format!("{{\"len\":{}}}", case.bytes().len()))
        .expect("check");

    assert!(!report.is_ok()); // it wrote a golden
    assert_eq!(
        std::fs::read_to_string(dir.path().join("golden/page.json")).expect("golden"),
        "{\"len\":8}\n"
    );
}

#[test]
fn a_missing_corpus_directory_is_an_error_with_the_path_in_it() {
    let missing = Path::new("/nonexistent/tome/corpus/normalization");

    let error = Golden::new(missing)
        .check(shout)
        .expect_err("should not silently pass");

    assert!(
        error.to_string().contains("normalization"),
        "the error must name the corpus it could not read: {error}"
    );
}
