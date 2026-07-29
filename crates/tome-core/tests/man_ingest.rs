//! Manual pages, ingested end to end (S2-11, spec P2-013).
//!
//! These run `mandoc` for real, against pages written into a temporary
//! directory. Stubbing it would test a mock's idea of roff, and roff is
//! exactly the part nobody should be reimplementing — see `man.rs`.
//!
//! The whole suite skips when `/usr/bin/mandoc` is absent rather than failing:
//! Tome ships macOS only, where it is present, but the tests should not turn
//! a Linux checkout red for a platform the product does not claim.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use tome_core::man::{discover, ingest, render, ManPage, SECTIONS};
use tome_core::model::SourceId;

fn mandoc_present() -> bool {
    Path::new("/usr/bin/mandoc").exists()
}

/// A minimal but real mdoc page.
fn page_source(name: &str, section: u8, description: &str, see_also: &[&str]) -> String {
    let refs = see_also
        .iter()
        .map(|r| {
            let (n, s) = r.split_once('(').expect("reference");
            format!(".Xr {} {}", n, s.trim_end_matches(')'))
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        ".Dd January 1, 2026\n\
         .Dt {upper} {section}\n\
         .Os\n\
         .Sh NAME\n\
         .Nm {name}\n\
         .Nd {description}\n\
         .Sh DESCRIPTION\n\
         The\n\
         .Nm\n\
         utility does a thing worth documenting.\n\
         .Sh SEE ALSO\n\
         {refs}\n",
        upper = name.to_uppercase(),
    )
}

struct Library {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

fn library(pages: &[(&str, u8, &str, &[&str])]) -> Library {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    for (name, section, description, see_also) in pages {
        let sub = root.join(format!("man{section}"));
        std::fs::create_dir_all(&sub).expect("mkdir");
        std::fs::write(
            sub.join(format!("{name}.{section}")),
            page_source(name, *section, description, see_also),
        )
        .expect("write page");
    }
    Library { _dir: dir, root }
}

fn source() -> SourceId {
    SourceId::new("man").expect("source id")
}

#[test]
fn a_manual_page_becomes_a_readable_document() {
    if !mandoc_present() {
        return;
    }
    let lib = library(&[("widget", 1, "do a thing worth documenting", &[])]);
    let found = discover(std::slice::from_ref(&lib.root), &SECTIONS);
    assert_eq!(found.len(), 1);

    let (docset, errors) = ingest(&found, &source(), &mut |_, _| {});
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(docset.pages.len(), 1);

    let page = &docset.pages[0];
    // The section is part of the identity: a library with three `printf`
    // pages has told the reader nothing without it.
    assert_eq!(page.meta.title, "widget(1)");
    assert_eq!(page.meta.path.as_str(), "man1/widget.1.html");

    let text = format!("{:?}", page.body);
    assert!(text.contains("DESCRIPTION"), "{text}");
    assert!(text.contains("worth documenting"), "{text}");
}

#[test]
fn a_cross_reference_to_an_installed_page_becomes_a_link() {
    if !mandoc_present() {
        return;
    }
    let lib = library(&[
        ("widget", 1, "make widgets", &["gadget(1)"]),
        ("gadget", 1, "make gadgets", &[]),
    ]);
    let found = discover(std::slice::from_ref(&lib.root), &SECTIONS);
    let (docset, _) = ingest(&found, &source(), &mut |_, _| {});

    let widget = docset
        .pages
        .iter()
        .find(|p| p.meta.title == "widget(1)")
        .expect("widget page");
    let text = format!("{:?}", widget.body);
    assert!(
        text.contains("man1/gadget.1.html"),
        "the SEE ALSO reference should link to the installed page: {text}"
    );
}

#[test]
fn a_cross_reference_to_a_page_that_is_not_installed_stays_plain() {
    // A link that looks like it would work and does not is worse than text.
    if !mandoc_present() {
        return;
    }
    let lib = library(&[("widget", 1, "make widgets", &["notinstalled(9)"])]);
    let found = discover(std::slice::from_ref(&lib.root), &SECTIONS);
    let (docset, _) = ingest(&found, &source(), &mut |_, _| {});

    let text = format!("{:?}", docset.pages[0].body);
    assert!(!text.contains("man9/notinstalled"), "{text}");
    // ...but the reference is still visible as text.
    assert!(text.contains("notinstalled"), "{text}");
}

#[test]
fn sections_are_kept_separate() {
    if !mandoc_present() {
        return;
    }
    let lib = library(&[
        ("printf", 1, "the shell command", &[]),
        ("printf", 3, "the library function", &[]),
    ]);
    let found = discover(std::slice::from_ref(&lib.root), &SECTIONS);
    assert_eq!(found.len(), 2, "{found:?}");

    let (docset, _) = ingest(&found, &source(), &mut |_, _| {});
    let paths: Vec<&str> = docset.pages.iter().map(|p| p.meta.path.as_str()).collect();
    assert!(paths.contains(&"man1/printf.1.html"), "{paths:?}");
    assert!(paths.contains(&"man3/printf.3.html"), "{paths:?}");
}

#[test]
fn a_gzipped_page_renders_the_same_as_a_plain_one() {
    // Linux ships compressed pages; macOS does not. Both must work.
    if !mandoc_present() || !Path::new("/usr/bin/gzip").exists() {
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let sub = dir.path().join("man1");
    std::fs::create_dir_all(&sub).expect("mkdir");

    let source_text = page_source("widget", 1, "make widgets", &[]);
    let plain = sub.join("widget.1");
    std::fs::write(&plain, &source_text).expect("write");

    let gz = sub.join("gadget.1.gz");
    let compressed = std::process::Command::new("/usr/bin/gzip")
        .arg("-c")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write as _;
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(page_source("gadget", 1, "make gadgets", &[]).as_bytes())?;
            child.wait_with_output()
        })
        .expect("gzip");
    std::fs::write(&gz, compressed.stdout).expect("write gz");

    let plain_html = render(&ManPage {
        name: "widget".to_owned(),
        section: 1,
        file: plain,
    })
    .expect("render plain");
    let gz_html = render(&ManPage {
        name: "gadget".to_owned(),
        section: 1,
        file: gz,
    })
    .expect("render gz");

    // Whitespace is collapsed before comparing: mandoc hard-wraps its output,
    // so `<span class="Nd">make\n    gadgets</span>` is what a two-word
    // description looks like in the raw HTML. `man::description` collapses it
    // the same way.
    let flat = |html: &str| html.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(flat(&plain_html).contains("make widgets"), "{plain_html}");
    assert!(flat(&gz_html).contains("make gadgets"), "{gz_html}");
}

#[test]
fn a_file_that_is_not_a_manual_page_does_not_sink_the_ingest() {
    // `/usr/share/man/man1` really does contain `[.1` and `@TSET@.1`, plus
    // README files and editor droppings. One of them must not cost two
    // thousand pages.
    if !mandoc_present() {
        return;
    }
    let lib = library(&[("widget", 1, "make widgets", &[])]);
    let man1 = lib.root.join("man1");
    std::fs::write(man1.join("README"), b"not a page").expect("write");
    std::fs::write(man1.join(".DS_Store"), b"\0\0").expect("write");
    std::fs::write(man1.join("notes.txt"), b"nor is this").expect("write");

    let found = discover(std::slice::from_ref(&lib.root), &SECTIONS);
    assert_eq!(found.len(), 1, "{found:?}");

    let (docset, errors) = ingest(&found, &source(), &mut |_, _| {});
    assert_eq!(docset.pages.len(), 1);
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn progress_is_reported_for_every_page() {
    if !mandoc_present() {
        return;
    }
    let lib = library(&[
        ("a", 1, "first", &[]),
        ("b", 1, "second", &[]),
        ("c", 1, "third", &[]),
    ]);
    let found = discover(std::slice::from_ref(&lib.root), &SECTIONS);

    let mut seen = Vec::new();
    let (_, _) = ingest(&found, &source(), &mut |done, total| {
        seen.push((done, total))
    });
    assert_eq!(seen, vec![(1, 3), (2, 3), (3, 3)]);
}

#[test]
fn the_real_system_manual_is_discoverable() {
    // The one test that touches the machine's own manual. It asserts only
    // that discovery finds *something* and that the commonest page on any
    // Unix renders, because the contents of /usr/share/man are not Tome's to
    // predict.
    if !mandoc_present() || !Path::new("/usr/share/man/man1").exists() {
        return;
    }
    let found = discover(&[PathBuf::from("/usr/share/man")], &SECTIONS);
    assert!(found.len() > 10, "found only {} pages", found.len());

    let Some(ls) = found.iter().find(|p| p.name == "ls" && p.section == 1) else {
        return;
    };
    let html = render(ls).expect("render ls(1)");
    assert!(html.contains("NAME"), "{}", &html[..html.len().min(400)]);
}
