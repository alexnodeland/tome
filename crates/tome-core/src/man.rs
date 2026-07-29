//! Unix manual pages (S2-11, spec P2-013).
//!
//! # Nothing here is fetched
//!
//! Man pages are already on the machine. There is no crawl, no `robots.txt`,
//! no rate limit and no SSRF surface — the whole of `fetch` is bypassed, which
//! is why [`crate::crawl::Crawler::new`] returns `None` for this source type
//! rather than trying.
//!
//! What replaces it is a different risk: this module runs an **external
//! program**. `mandoc` is invoked by absolute path with an argument vector and
//! never through a shell, so a directory named `; rm -rf ~` is a directory
//! name. The paths themselves come from the source configuration, which is a
//! file the user wrote.
//!
//! # Why `mandoc -T html` rather than parsing roff
//!
//! roff is a typesetting language with macros, conditionals and arithmetic; a
//! man page is a *program*. `mandoc` is the reference implementation, ships on
//! macOS, and emits semantic HTML with the section structure intact —
//! `<h1 class="Sh" id="NAME">`, `<a class="Xr">ctype(3)</a>` — which the
//! existing parse and normalize pipeline already understands. Writing a roff
//! interpreter to avoid one process spawn would be trading a solved problem
//! for an unsolved one.
//!
//! # Cross-references have to be synthesised
//!
//! mandoc marks a cross-reference as `<a class="Xr">ctype(3)</a>` **with no
//! `href`** — it has no way to know where the target will live. Turning those
//! into working links is this module's job, and it only does so when the
//! target is a page that was actually discovered: a link to a page the user
//! does not have installed is worse than plain text, because it looks like it
//! would work.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};

/// Where `mandoc` lives.
///
/// Absolute, and not looked up on `PATH`: this is a program Tome executes, and
/// resolving it through an environment variable the user's shell controls is
/// how "documentation reader" becomes "arbitrary code execution".
const MANDOC: &str = "/usr/bin/mandoc";

/// Sections 1–8, the conventional range. P2-013 asks for section-aware
/// organisation and this is the set it means.
pub const SECTIONS: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

/// One manual page found on disk.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ManPage {
    /// `ls`, `printf`, `ssh_config`.
    pub name: String,
    /// 1–8. Kept as the number rather than the directory name so `man1` and
    /// `man1p` do not become two sections.
    pub section: u8,
    /// The file on disk, possibly `.gz`.
    pub file: PathBuf,
}

impl ManPage {
    /// The library path this page is stored under: `man1/ls.1.html`.
    ///
    /// Section-first, so a library browser groups by section without needing
    /// to know anything about man pages, and so `ls(1)` and a hypothetical
    /// `ls(3)` cannot collide.
    pub fn library_path(&self) -> String {
        format!("man{}/{}.{}.html", self.section, self.name, self.section)
    }

    /// `ls(1)` — how a cross-reference names it.
    pub fn reference(&self) -> String {
        format!("{}({})", self.name, self.section)
    }
}

/// Find every manual page under `roots`, restricted to `sections`.
///
/// Directories that do not exist are skipped rather than failing the run: a
/// configuration listing `/usr/local/share/man` is correct on a machine that
/// has Homebrew and harmless on one that does not.
pub fn discover(roots: &[PathBuf], sections: &[u8]) -> Vec<ManPage> {
    let wanted: BTreeSet<u8> = sections.iter().copied().collect();
    let mut found = Vec::new();

    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            tracing::debug!(path = %root.display(), "man directory not present, skipping");
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            // `man1`, `man3`, and also `man1p`/`man3f` on some systems — the
            // digit immediately after `man` is the section.
            let Some(section) = dir
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_prefix("man"))
                .and_then(|rest| rest.chars().next())
                .and_then(|c| c.to_digit(10))
                .and_then(|d| u8::try_from(d).ok())
            else {
                continue;
            };
            if !wanted.contains(&section) {
                continue;
            }

            let Ok(pages) = std::fs::read_dir(&dir) else {
                continue;
            };
            for page in pages.flatten() {
                let file = page.path();
                if !file.is_file() {
                    continue;
                }
                if let Some(name) = page_name(&file) {
                    found.push(ManPage {
                        name,
                        section,
                        file,
                    });
                }
            }
        }
    }

    // Sorted and deduplicated by (section, name): `MANPATH` routinely lists
    // directories that shadow one another, and the first one found wins — the
    // same rule `man` itself applies.
    found.sort();
    found.dedup_by(|a, b| a.section == b.section && a.name == b.name);
    found
}

/// The page's name, from `ls.1` or `ls.1.gz`.
///
/// Returns `None` for anything that is not a manual page: `README`, a stray
/// `.DS_Store`, or the `[.1` and `@TSET@.1` files that really do exist in
/// `/usr/share/man/man1` and are not pages anyone can open.
fn page_name(file: &Path) -> Option<String> {
    let name = file.file_name()?.to_str()?;
    let stem = name.strip_suffix(".gz").unwrap_or(name);
    let (base, section) = stem.rsplit_once('.')?;
    // The extension must start with a section digit, or `foo.conf` in a man
    // directory becomes a page called `foo`.
    if !section.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    if base.is_empty() || base.starts_with('@') || base.starts_with('[') {
        return None;
    }
    Some(base.to_owned())
}

/// Render one page to HTML with `mandoc`.
///
/// `.gz` pages are decompressed by `gzip -dc` into `mandoc`'s stdin rather
/// than by adding a compression crate: it is one more process on a path that
/// already spawns one, and it keeps the decompressor out of Tome's own address
/// space, which for an attacker-supplied archive is the safer side of the line.
pub fn render(page: &ManPage) -> Result<String> {
    let compressed = page.file.extension().is_some_and(|e| e == "gz");

    let output = if compressed {
        let decompressed = Command::new("/usr/bin/gzip")
            .arg("-dc")
            .arg(&page.file)
            .output()
            .map_err(|source| Error::Man {
                message: format!("running gzip: {source}"),
            })?;
        if !decompressed.status.success() {
            return Err(Error::Man {
                message: format!("gzip could not read {}", page.file.display()),
            });
        }
        run_mandoc(Some(&decompressed.stdout), None)?
    } else {
        run_mandoc(None, Some(&page.file))?
    };

    Ok(output)
}

fn run_mandoc(stdin: Option<&[u8]>, file: Option<&Path>) -> Result<String> {
    use std::io::Write as _;
    use std::process::Stdio;

    let mut command = Command::new(MANDOC);
    // `-T html` and nothing else. macOS's mandoc has no `-Q`: the flag exists
    // in OpenBSD's build and adding it here made *every* page render as an
    // empty document, because mandoc printed a usage message to stderr and
    // exited without reading anything. Warnings go to stderr, which is
    // discarded below — plenty of shipped pages have them.
    command.arg("-T").arg("html");
    if let Some(file) = file {
        command.arg(file);
    }

    let mut child = command
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| Error::Man {
            message: format!(
                "running {MANDOC}: {source}. Man page support needs mandoc, which ships \
                 with macOS."
            ),
        })?;

    if let Some(bytes) = stdin {
        // Ignore a broken pipe: mandoc closing stdin early means it has
        // decided it has enough, not that the render failed.
        if let Some(mut sink) = child.stdin.take() {
            let _ = sink.write_all(bytes);
        }
    }

    let output = child.wait_with_output().map_err(|source| Error::Man {
        message: format!("waiting for {MANDOC}: {source}"),
    })?;

    // mandoc exits non-zero for warnings as well as errors, so the exit code
    // is not the test — empty output is.
    let html = String::from_utf8_lossy(&output.stdout).into_owned();
    if html.trim().is_empty() {
        return Err(Error::Man {
            message: "mandoc produced no output".to_owned(),
        });
    }
    Ok(html)
}

/// Give mandoc's cross-references an `href`, where the target exists.
///
/// mandoc emits `<a class="Xr">ctype(3)</a>` with no destination. This
/// rewrites it to `<a class="Xr" href="man3/ctype.3.html">ctype(3)</a>` — but
/// **only** when `ctype(3)` is one of `known`. A link to a page the user does
/// not have installed looks like it would work and does not, which is worse
/// than the plain text mandoc emitted.
pub fn link_cross_references(html: &str, known: &BTreeSet<String>) -> String {
    const OPEN: &str = "<a class=\"Xr\">";
    let mut out = String::with_capacity(html.len());
    let mut rest = html;

    while let Some(at) = rest.find(OPEN) {
        let (before, tail) = rest.split_at(at);
        out.push_str(before);
        let body = &tail[OPEN.len()..];

        let Some(end) = body.find("</a>") else {
            // Unterminated: emit what is left verbatim rather than losing it.
            out.push_str(tail);
            return out;
        };
        let text = &body[..end];

        match parse_reference(text)
            .filter(|(name, section)| known.contains(&format!("{name}({section})")))
        {
            Some((name, section)) => {
                // The text is mandoc's, and mandoc escapes it. It is re-emitted
                // unchanged; the href is built from the parsed name, which
                // `parse_reference` has already restricted to identifier
                // characters.
                out.push_str(&format!(
                    "<a class=\"Xr\" href=\"man{section}/{name}.{section}.html\">{text}</a>"
                ));
            }
            None => out.push_str(&format!("{OPEN}{text}</a>")),
        }
        rest = &body[end + 4..];
    }

    out.push_str(rest);
    out
}

/// `ctype(3)` → `("ctype", 3)`.
///
/// Rejects anything whose name is not made of the characters a page name can
/// contain, so nothing that reaches an `href` can carry a quote, a slash or a
/// path traversal.
fn parse_reference(text: &str) -> Option<(String, u8)> {
    let (name, rest) = text.trim().split_once('(')?;
    let section = rest.strip_suffix(')')?;
    let section: u8 = section.parse().ok()?;
    if !SECTIONS.contains(&section) {
        return None;
    }
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '+')
    {
        return None;
    }
    Some((name.to_owned(), section))
}

/// The one-line description from the NAME section: `ls — list directory
/// contents` yields `list directory contents`.
///
/// This is what a result list shows under the title, and what makes a man page
/// findable by what it *does* rather than only by its name — P2-013's "index
/// NAME section for search".
pub fn description(html: &str) -> Option<String> {
    let at = html.find("class=\"Nd\"")?;
    let after = &html[at..];
    let start = after.find('>')? + 1;
    let end = after[start..].find('<')? + start;
    let text = decode_entities(&after[start..end]);
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Decode the handful of entities mandoc emits in a description.
///
/// Deliberately not a general HTML entity decoder: this reads one short
/// attribute-free run of text, and a full decoder here would be a second,
/// less-tested path alongside the parser that handles the rest of the page.
fn decode_entities(text: &str) -> String {
    let mut out = text.to_owned();
    for (entity, replacement) in [
        ("&#x2014;", "\u{2014}"),
        ("&#x2013;", "\u{2013}"),
        ("&mdash;", "\u{2014}"),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&nbsp;", " "),
    ] {
        if out.contains(entity) {
            out = out.replace(entity, replacement);
        }
    }
    out
}

/// Render every discovered page into a [`DocSet`], ready for the same
/// storing, indexing and rendering the crawler's output goes through.
///
/// The two passes are not an accident. Cross-references can only be linked
/// once the whole set is known — `ls(1)` referring to `chflags(1)` is a link
/// if that page was discovered and plain text if it was not — which is the
/// same reason `pipeline::relink` runs after the crawl rather than during it.
///
/// A page `mandoc` cannot render is reported and skipped. Manual page
/// collections contain broken and non-page files (`/usr/share/man/man1` on
/// macOS has both `[.1` and `@TSET@.1` right now), and one of them must not
/// sink an ingest of two thousand.
pub fn ingest(
    pages: &[ManPage],
    source: &crate::model::SourceId,
    on_page: &mut dyn FnMut(usize, usize),
) -> (crate::model::DocSet, Vec<String>) {
    use crate::model::{ContentHash, DocPage, Page, PagePath};

    let known: BTreeSet<String> = pages.iter().map(ManPage::reference).collect();
    let mut out = Vec::new();
    let mut errors = Vec::new();

    for (index, page) in pages.iter().enumerate() {
        on_page(index + 1, pages.len());

        let html = match render(page) {
            Ok(html) => html,
            Err(error) => {
                // The path is a system path, not reading history.
                errors.push(format!("{}: {error}", page.file.display()));
                continue;
            }
        };
        let html = link_cross_references(&html, &known);

        // A synthetic base: man pages have no URL, and relative-link
        // resolution needs *something* that parses. Nothing is ever fetched
        // from it, and `relink` rewrites what survives to library paths.
        let Ok(base) = url::Url::parse(&format!("man:///{}", page.library_path())) else {
            errors.push(format!("{}: could not build a base URL", page.reference()));
            continue;
        };

        // The generic path, with no profile: mandoc's HTML comes from a
        // typesetter rather than a documentation generator, and its furniture
        // (`class="permalink"`) is already a self-permalink the parser
        // unwraps.
        let parsed = crate::parse::parse_page(&html, &base, None);
        let normalized = crate::normalize::normalize(parsed.body, &base);

        let Ok(path) = PagePath::new(page.library_path()) else {
            errors.push(format!("{}: invalid library path", page.reference()));
            continue;
        };

        let meta = Page::new(
            source.clone(),
            path,
            // `ls(1)` rather than `ls`: the section is part of a page's
            // identity, and a result list showing three pages called `printf`
            // has told the reader nothing. mandoc's own `<h1>` is the section
            // banner ("LS(1)" shouted), so the title is built here rather than
            // taken from `normalized.title`.
            page.reference(),
            ContentHash::from_digest(crate::hash::sha256(html.as_bytes())),
        );

        out.push(DocPage {
            meta,
            body: normalized.body,
        });
    }

    (crate::model::DocSet::new(out, Vec::new()), errors)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn known(refs: &[&str]) -> BTreeSet<String> {
        refs.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn a_page_knows_where_it_is_stored_and_how_it_is_referenced() {
        let page = ManPage {
            name: "ls".to_owned(),
            section: 1,
            file: PathBuf::from("/usr/share/man/man1/ls.1"),
        };
        assert_eq!(page.library_path(), "man1/ls.1.html");
        assert_eq!(page.reference(), "ls(1)");
    }

    #[test]
    fn page_names_come_off_both_plain_and_gzipped_files() {
        assert_eq!(page_name(Path::new("/m/man1/ls.1")).as_deref(), Some("ls"));
        assert_eq!(
            page_name(Path::new("/m/man1/ls.1.gz")).as_deref(),
            Some("ls")
        );
        assert_eq!(
            page_name(Path::new("/m/man5/ssh_config.5")).as_deref(),
            Some("ssh_config")
        );
    }

    #[test]
    fn things_that_are_not_pages_are_not_pages() {
        // Every one of these really exists in a man directory. `[.1` and
        // `@TSET@.1` are in `/usr/share/man/man1` on macOS right now.
        for path in [
            "/m/man1/README",
            "/m/man1/.DS_Store",
            "/m/man1/[.1",
            "/m/man1/@TSET@.1",
            "/m/man1/foo.conf",
            "/m/man1/.1",
        ] {
            assert_eq!(page_name(Path::new(path)), None, "{path}");
        }
    }

    #[test]
    fn a_cross_reference_to_an_installed_page_becomes_a_link() {
        let html = r#"<p>See <a class="Xr">ctype(3)</a> for details.</p>"#;
        let out = link_cross_references(html, &known(&["ctype(3)"]));
        assert!(
            out.contains(r#"<a class="Xr" href="man3/ctype.3.html">ctype(3)</a>"#),
            "{out}"
        );
    }

    #[test]
    fn a_cross_reference_to_a_page_that_is_not_installed_stays_plain() {
        // A link that looks like it would work and does not is worse than the
        // text mandoc emitted.
        let html = r#"<p>See <a class="Xr">notinstalled(9)</a>.</p>"#;
        let out = link_cross_references(html, &known(&["ctype(3)"]));
        assert_eq!(out, html);
    }

    #[test]
    fn several_cross_references_in_one_page_are_all_rewritten() {
        let html =
            r#"<a class="Xr">ls(1)</a> and <a class="Xr">cp(1)</a> and <a class="Xr">gone(1)</a>"#;
        let out = link_cross_references(html, &known(&["ls(1)", "cp(1)"]));
        assert_eq!(out.matches("href=").count(), 2, "{out}");
        assert!(out.contains(r#"<a class="Xr">gone(1)</a>"#), "{out}");
    }

    #[test]
    fn a_reference_that_is_not_a_reference_is_left_alone() {
        for text in ["not a reference", "ls(99)", "ls()", "(1)", "ls(1", "ls 1"] {
            let html = format!(r#"<a class="Xr">{text}</a>"#);
            assert_eq!(
                link_cross_references(&html, &known(&[text])),
                html,
                "{text}"
            );
        }
    }

    #[test]
    fn a_reference_name_can_never_carry_path_or_quote_characters() {
        // The href is built from the parsed name, so this is where a
        // traversal or an attribute break would have to get in.
        for text in [
            "../../etc/passwd(1)",
            r#"a"onerror=x(1)"#,
            "a/b(1)",
            "a b(1)",
            "a>b(1)",
        ] {
            assert_eq!(parse_reference(text), None, "{text}");
        }
    }

    #[test]
    fn an_unterminated_anchor_does_not_lose_the_rest_of_the_page() {
        let html = r#"before <a class="Xr">ls(1) and then some text"#;
        let out = link_cross_references(html, &known(&["ls(1)"]));
        assert!(out.contains("and then some text"), "{out}");
    }

    #[test]
    fn the_name_section_yields_the_one_line_description() {
        let html = r#"<h1 class="Sh" id="NAME">NAME</h1>
            <p class="Pp"><code class="Nm">ls</code> &#x2014;
            <span class="Nd">list directory
            contents</span></p>"#;
        assert_eq!(
            description(html).as_deref(),
            Some("list directory contents")
        );
    }

    #[test]
    fn a_page_with_no_name_section_has_no_description() {
        assert_eq!(description("<p>nothing here</p>"), None);
        assert_eq!(description(r#"<span class="Nd"></span>"#), None);
        assert_eq!(description(""), None);
    }

    #[test]
    fn discovery_skips_directories_that_do_not_exist() {
        // A configuration naming `/usr/local/share/man` is correct on a
        // machine with Homebrew and harmless on one without.
        let found = discover(&[PathBuf::from("/nonexistent/man/root")], &SECTIONS);
        assert!(found.is_empty());
    }

    #[test]
    fn discovery_finds_pages_and_honours_the_section_filter() {
        let dir = tempfile::tempdir().expect("tempdir");
        for (section, name) in [("man1", "ls.1"), ("man1", "cp.1"), ("man3", "printf.3")] {
            let sub = dir.path().join(section);
            std::fs::create_dir_all(&sub).expect("mkdir");
            std::fs::write(sub.join(name), b".Dd\n").expect("write");
        }

        let all = discover(&[dir.path().to_path_buf()], &SECTIONS);
        assert_eq!(all.len(), 3, "{all:?}");
        // Sorted by section then name, which is what a library sidebar wants.
        assert_eq!(all[0].reference(), "cp(1)");

        let only_three = discover(&[dir.path().to_path_buf()], &[3]);
        assert_eq!(only_three.len(), 1);
        assert_eq!(only_three[0].reference(), "printf(3)");
    }

    #[test]
    fn a_page_shadowed_by_an_earlier_path_appears_once() {
        // `MANPATH` routinely lists directories that shadow one another, and
        // the first one found wins — the rule `man` itself applies.
        let first = tempfile::tempdir().expect("tempdir");
        let second = tempfile::tempdir().expect("tempdir");
        for root in [&first, &second] {
            let sub = root.path().join("man1");
            std::fs::create_dir_all(&sub).expect("mkdir");
            std::fs::write(sub.join("ls.1"), b".Dd\n").expect("write");
        }

        let found = discover(
            &[first.path().to_path_buf(), second.path().to_path_buf()],
            &SECTIONS,
        );
        assert_eq!(found.len(), 1, "{found:?}");
    }
}
