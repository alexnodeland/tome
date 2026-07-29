# Normalization corpus — sources

Per `corpus/README.md`, every committed input records its URL, retrieval
date, licence, and modifications — the SPIKE-010 licence gate for the golden
corpus (which ships publicly, so redistribution rules apply).

**26 cases across six documentation platforms.** S1-8's acceptance criterion
was "normalization judged across ≥ 20 real sites spanning every target
platform"; this is that suite.

## Modifications

**None, to any file.** Every page is the bytes the server returned, saved
verbatim: no truncation, no reformatting, no stripping. Prettier and ESLint
are configured to leave this directory alone (`.prettierignore`,
`eslint.config.js`) precisely so that stays true — a corpus input that has
been tidied no longer records what the pipeline was given.

All were fetched over HTTPS on the date below with the same self-identifying
user agent the crawler uses (`Tome/0.0.0 (+https://github.com/alexnodeland/tome)`),
one request at a time. `robots.txt` was checked for every host first;
`nodejs.org` disallows `/docs/` and allows `/api/`, which is why the Node
cases are `/api/` pages.

## Licences, as stated by each source

| Source | Licence | Verified from |
|---|---|---|
| docs.python.org | **PSF-2.0** (code samples additionally 0BSD) | The page footer: "This page is licensed under the Python Software Foundation License Version 2." |
| doc.rust-lang.org (Cargo book, `std`) | **MIT OR Apache-2.0** | `rust-lang/cargo` README § License: "distributed under the terms of both the MIT license and the Apache License (Version 2.0)" |
| nodejs.org/api | **MIT** | `nodejs/node` `LICENSE` |
| kubernetes.io | **CC-BY-4.0** | The page footer: "Documentation Distributed under CC BY 4.0" |
| go.dev | **CC-BY-4.0** (code samples BSD) | `go.dev/copyright`: "the contents of this site are licensed under the Creative Commons Attribution 4.0 License, and code is licensed under a BSD license" |

Every one of these permits alteration and redistribution, which is the gate in
`corpus/README.md`. **No CC-BY-SA input is present** — MDN was considered and
left out, so the derived goldens carry no share-alike obligation and this file
does not have to track one. If MDN is ever added, that changes and this
paragraph must change with it.

Attribution for the CC-BY sources is this table: each names the work, its
origin, and its licence, and the files are unmodified.

## Inputs

Retrieved **2026-07-29** (UTC). Each case is named
`<platform>-<what-is-interesting-about-it>`.

### Sphinx — `docs.python.org` (PSF-2.0)

The Stage 1 target platform. `json-api` is the case the whole vertical slice
rests on: Sphinx renders every API entry as a `<dl>`, and 26 of them are in
that one page.

| File | URL |
|---|---|
| `sphinx-python-controlflow.html` | https://docs.python.org/3/tutorial/controlflow.html |
| `sphinx-python-datastructures.html` | https://docs.python.org/3/tutorial/datastructures.html |
| `sphinx-python-errors.html` | https://docs.python.org/3/tutorial/errors.html |
| `sphinx-python-inputoutput.html` | https://docs.python.org/3/tutorial/inputoutput.html |
| `sphinx-python-json-api.html` | https://docs.python.org/3/library/json.html |
| `sphinx-python-modules.html` | https://docs.python.org/3/tutorial/modules.html |

### mdBook — the Cargo Book (MIT OR Apache-2.0)

`manifest` and `environment-variables` are the table-heavy cases; every
heading on these pages is wrapped in a self-referential permalink, which is
why `unwrap_self_permalink` exists.

| File | URL |
|---|---|
| `mdbook-cargo-build-command.html` | https://doc.rust-lang.org/cargo/commands/cargo-build.html |
| `mdbook-cargo-dependencies.html` | https://doc.rust-lang.org/cargo/guide/dependencies.html |
| `mdbook-cargo-environment-variables.html` | https://doc.rust-lang.org/cargo/reference/environment-variables.html |
| `mdbook-cargo-features.html` | https://doc.rust-lang.org/cargo/reference/features.html |
| `mdbook-cargo-glossary.html` | https://doc.rust-lang.org/cargo/appendix/glossary.html |
| `mdbook-cargo-manifest.html` | https://doc.rust-lang.org/cargo/reference/manifest.html |

### rustdoc — the Rust standard library (MIT OR Apache-2.0)

A generated-API layout unlike any of the others: dense method lists, trait
implementations, and `impl-From<T>-for-T`-shaped anchors — the ids the
sanitizer's denylist was rewritten to preserve.

| File | URL |
|---|---|
| `rustdoc-std-iter-module.html` | https://doc.rust-lang.org/std/iter/index.html |
| `rustdoc-std-once.html` | https://doc.rust-lang.org/std/sync/struct.Once.html |
| `rustdoc-std-read-to-string.html` | https://doc.rust-lang.org/std/fs/fn.read_to_string.html |
| `rustdoc-std-vec-module.html` | https://doc.rust-lang.org/std/vec/index.html |

### Node.js API docs (MIT)

The table-heavy platform, and the one whose permalinks are `#` rather than a
pilcrow — which is how every Node page in this corpus came to be titled `OS#`
until `PERMALINK_MARKERS` grew past Sphinx's glyph.

| File | URL |
|---|---|
| `node-api-os.html` | https://nodejs.org/api/os.html |
| `node-api-path.html` | https://nodejs.org/api/path.html |
| `node-api-querystring.html` | https://nodejs.org/api/querystring.html |

### Hugo — kubernetes.io (CC-BY-4.0)

The largest input in the corpus by some way, and the one with real images.

| File | URL |
|---|---|
| `k8s-concepts-pods.html` | https://kubernetes.io/docs/concepts/workloads/pods/ |

### go.dev (CC-BY-4.0)

Hand-written prose with heavy code interleaving, plus a definition-list
reference page.

| File | URL |
|---|---|
| `go-doc-comment.html` | https://go.dev/doc/comment |
| `go-modules-gomod-ref.html` | https://go.dev/doc/modules/gomod-ref |
| `go-tutorial-getting-started.html` | https://go.dev/doc/tutorial/getting-started |

### The repository's own Sphinx fixture (MIT OR Apache-2.0)

Kept alongside the real pages. They are small enough to read in full, which
makes them the cases to reach for when a diff needs to be understood rather
than merely reviewed.

| File | Source |
|---|---|
| `sphinx-index.html` | `tome-testkit/fixtures/sphinx-example/index.html` |
| `sphinx-api-reference.html` | `tome-testkit/fixtures/sphinx-example/api/reference.html` |
| `sphinx-guide.html` | `tome-testkit/fixtures/sphinx-example/guide/index.html` |

## What the goldens are not

They are a record of what the pipeline *currently* produces, not a statement
that it is right. Some known imperfections are baked in and visible in the
output — go.dev breadcrumbs and rustdoc's `1.26.0 · Source` chrome survive
inside the content root, for instance. That is the point: they are recorded,
so improving them shows up as a reviewable diff rather than as an
unmeasurable "looks better".
