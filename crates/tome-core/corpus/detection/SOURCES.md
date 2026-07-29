# Detection corpus — sources

Per [`corpus/README.md`](../README.md), every committed page records where it
came from and under which licence it is redistributed. This is the SPIKE-010
licence gate applied to the platform-detection corpus (S2-9, spec P2-020),
which ships publicly like the rest of the repository.

**128 homepages across six documentation platforms**, fetched 2026-07-29 by
[`scripts/fetch-detection-corpus.mjs`](../../../../scripts/fetch-detection-corpus.mjs).

## How these were produced

The script honours `robots.txt` for every host, issues one request at a time
with a 400 ms delay, and uses the crawler's self-identifying user agent
(`Tome/0.0.0 (+https://github.com/alexnodeland/tome)`). One host —
`docs.astropy.org` — disallowed the path and was skipped rather than fetched
anyway.

Each fixture is **truncated**: the `<head>` in full, the first 6 KB of the
`<body>`, and the last 3 KB, with the elision marked inline. The foot matters
as much as the head — "Built with MkDocs", "Made with Docusaurus" and mdBook's
script tags all sit near the end of the document. Everything between is bytes
nothing reads. Response headers are filtered to the eleven a detector could
legitimately use; the rest identify the request rather than the site.

That truncation makes each fixture a derived work rather than a copy, and every
licence below permits redistribution of modified works.

## How the labels were decided

**A label answers "which scraper should handle this site", not "which program
emitted the HTML".** Those come apart, and when they do the scraper wins.

Labels are asserted by a person from knowledge of each project, **not derived
from the page** — a corpus labelled by the same markers the detector reads
would score the detector against itself. Where a page self-identifies through
`<meta name="generator">` the fetch script cross-checks and reports
disagreements. On the first run it reported eight:

| Site | Labelled | Page says | Resolution |
|---|---|---|---|
| `docs.pydantic.dev` | mkdocs | Astro v7.1.3 | **Relabelled `generic`** — migrated, no Material markup remains |
| `swc.rs` | docusaurus | Rspress v2.0.16 | **Relabelled `generic`** — migrated; one stray `docusaurus` string survives in the page, which is exactly what a naive detector trips on |
| `fastapi.tiangolo.com` | mkdocs | zensical | **Kept `mkdocs`** — Zensical emits Material-for-MkDocs markup (`md-component`, `md-header`), so the MkDocs scraper handles it |
| `typer.tiangolo.com` | mkdocs | zensical | Kept, same reason |
| `sqlmodel.tiangolo.com` | mkdocs | zensical | Kept, same reason |
| `www.starlette.io` | mkdocs | zensical | Kept, same reason |
| `mkdocstrings.github.io` | mkdocs | zensical | Kept, same reason |
| `pdm-project.org` | mkdocs | zensical | Kept, same reason |

The cross-check paid for itself on its first run, and it is the reason the
`generator-meta` line is recorded in every fixture: a future re-capture will
report the next migration the same way.

**Three further labels were wrong, and building the detector (S2-10) found
them.** Counting markers across the corpus turned up fixtures whose evidence
contradicted their label:

| Site | Was | Now | Why |
|---|---|---|---|
| `docs.snapcraft.io` | generic | **sphinx** | Carries `documentation_options.js`. It *is* Sphinx; I had not checked |
| `mkdocs-macros-plugin.readthedocs.io` | sphinx | **mkdocs** | Labelled from the domain. **ReadTheDocs hosts MkDocs projects too** — the domain says who serves the pages, not what made them. This is now the corpus's counter-example to P2-014's first sketched rule |
| `graphql.org` | docusaurus | **generic** | Migrated to Next.js |

`classic.yarnpkg.com` was **dropped**: the archived Yarn v1 site was Docusaurus
v1, whose markers differ from v2's, and the page no longer serves anything that
establishes either. A fixture whose ground truth cannot be asserted is worse
than no fixture.

Two fixtures are deliberately *unclassifiable* from a homepage and are kept for
that reason. `docs.djangoproject.com` is Sphinx-built and post-processed into a
template carrying no Sphinx marker anywhere — P2-020's "heavily themed Sphinx"
— and `doc.rust-lang.org/rustdoc/` is the mdBook-built book *about* rustdoc,
full of the word "rustdoc". The first caps achievable accuracy honestly; the
second is why the detector matches on hashed asset paths rather than names.

## What is *not* here

**GitBook has no fixtures, deliberately.** It is a hosted product now, and its
public instances are companies' own documentation under no redistributable
licence. A probe of ten candidates (Uniswap, Chainlink, WalletConnect,
MetaMask, Truffle, Zapier and others) found all but GitBook's own docs had
migrated to Docusaurus. The harness reports an empty row rather than a score,
because nothing here measures GitBook detection — and none of P2-010..013 is a
GitBook scraper, so a precise GitBook label would buy nothing today.

The GNU manuals were dropped for the same class of reason: GFDL's
invariant-section conditions are more bookkeeping than one fixture is worth.

## Staleness

Every fixture records `captured:`. Documentation sites are rebuilt often — two
of these had already changed generator between the list being written and the
fetch running — so a corpus more than a year old is measuring a web that no
longer exists. Re-run the script; the cross-check will say what moved.

## Counts

| Platform | Fixtures |
|---|---|
| `sphinx` | 40 |
| `rustdoc` | 20 |
| `mdbook` | 20 |
| `mkdocs` | 12 |
| `docusaurus` | 10 |
| `generic` | 26 |
| `gitbook` | 0 — see above |

**Largest class is 40 of 128**, which the harness asserts stays under half: a corpus that was mostly one label would score well for a detector that always guessed it.

## Licences

- **MIT** — 48 fixtures
- **MIT OR Apache-2.0** — 34 fixtures
- **BSD-3-Clause** — 19 fixtures
- **Apache-2.0** — 9 fixtures
- **BSD-2-Clause** — 5 fixtures
- **CC-BY-SA-4.0** — 2 fixtures
- **PSF-2.0** — 2 fixtures
- **MPL-2.0** — 2 fixtures
- **curl licence (MIT-like)** — 1 fixture
- **IANA reserved example domain** — 1 fixture
- **CC-BY-4.0** — 1 fixture
- **Public domain** — 1 fixture
- **PostgreSQL licence** — 1 fixture
- **ISC** — 1 fixture
- **PSF-based (matplotlib)** — 1 fixture
- **MIT-CMU** — 1 fixture

`CC-BY-SA-4.0` appears above (Kubernetes, Redis, Snapcraft). Per
`corpus/README.md`, share-alike inputs make derived works share-alike too — but
the only thing derived here is a one-word label, so there is nothing further to
license. If that bookkeeping ever costs more than those three fixtures add,
drop them rather than the rule.

## Every fixture

| Platform | Fixture | URL | Captured | Licence | `generator` meta |
|---|---|---|---|---|---|
| `docusaurus` | `babeljs.io-docs` | https://babeljs.io/docs/ | 2026-07-29 | MIT | `(none)` |
| `docusaurus` | `create-react-app.dev-docs-getting-started` | https://create-react-app.dev/docs/getting-started/ | 2026-07-29 | MIT | `Docusaurus v2.0.0-beta.14` |
| `docusaurus` | `docusaurus.io-docs` | https://docusaurus.io/docs | 2026-07-29 | MIT | `(none)` |
| `docusaurus` | `jestjs.io-docs-getting-started` | https://jestjs.io/docs/getting-started | 2026-07-29 | MIT | `Docusaurus v3.10.1` |
| `docusaurus` | `prettier.io-docs` | https://prettier.io/docs/ | 2026-07-29 | MIT | `(none)` |
| `docusaurus` | `reactnative.dev-docs-getting-started` | https://reactnative.dev/docs/getting-started | 2026-07-29 | MIT | `(none)` |
| `docusaurus` | `redux.js.org` | https://redux.js.org/ | 2026-07-29 | MIT | `Docusaurus v3.6.3` |
| `docusaurus` | `socket.io-docs-v4` | https://socket.io/docs/v4/ | 2026-07-29 | MIT | `Docusaurus v2.4.3` |
| `docusaurus` | `typescript-eslint.io` | https://typescript-eslint.io/ | 2026-07-29 | MIT | `(none)` |
| `docusaurus` | `www.electronjs.org-docs-latest` | https://www.electronjs.org/docs/latest | 2026-07-29 | MIT | `(none)` |
| `generic` | `caddyserver.com-docs` | https://caddyserver.com/docs/ | 2026-07-29 | Apache-2.0 | `(none)` |
| `generic` | `curl.se-docs` | https://curl.se/docs/ | 2026-07-29 | curl licence (MIT-like) | `(none)` |
| `generic` | `docs.pydantic.dev-latest` | https://pydantic.dev/docs/validation/latest/get-started/ | 2026-07-29 | MIT | `Astro v7.1.3` |
| `generic` | `eslint.org-docs-latest` | https://eslint.org/docs/latest/ | 2026-07-29 | MIT | `(none)` |
| `generic` | `example.com` | https://example.com/ | 2026-07-29 | IANA reserved example domain | `(none)` |
| `generic` | `go.dev-doc` | https://go.dev/doc/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `generic` | `gohugo.io-documentation` | https://gohugo.io/documentation/ | 2026-07-29 | Apache-2.0 | `(none)` |
| `generic` | `graphql.org-learn` | https://graphql.org/learn/ | 2026-07-29 | MIT | `(none)` |
| `generic` | `jekyllrb.com-docs` | https://jekyllrb.com/docs/ | 2026-07-29 | MIT | `Jekyll v4.4.1` |
| `generic` | `kubernetes.io-docs-home` | https://kubernetes.io/docs/home/ | 2026-07-29 | CC-BY-4.0 | `(none)` |
| `generic` | `nodejs.org-en` | https://nodejs.org/en | 2026-07-29 | MIT | `(none)` |
| `generic` | `python-poetry.org-docs` | https://python-poetry.org/docs/ | 2026-07-29 | MIT | `(none)` |
| `generic` | `reactrouter.com` | https://reactrouter.com/ | 2026-07-29 | MIT | `(none)` |
| `generic` | `redis.io-docs-latest` | https://redis.io/docs/latest/ | 2026-07-29 | CC-BY-SA-4.0 | `Hugo 0.143.1` |
| `generic` | `sqlite.org-docs-html` | https://sqlite.org/docs.html | 2026-07-29 | Public domain | `(none)` |
| `generic` | `svelte.dev-docs-svelte-overview` | https://svelte.dev/docs/svelte/overview | 2026-07-29 | MIT | `(none)` |
| `generic` | `swc.rs-docs-getting-started` | https://swc.rs/docs/getting-started | 2026-07-29 | Apache-2.0 | `Rspress v2.0.16` |
| `generic` | `tauri.app-start` | https://tauri.app/start/ | 2026-07-29 | MIT OR Apache-2.0 | `Astro v7.1.3` |
| `generic` | `tsup.egoist.dev` | https://tsup.egoist.dev/ | 2026-07-29 | MIT | `(none)` |
| `generic` | `vitejs.dev-guide` | https://vite.dev/guide/ | 2026-07-29 | MIT | `VitePress v2.0.0-alpha.18` |
| `generic` | `vitepress.dev-guide-what-is-vitepress` | https://vitepress.dev/guide/what-is-vitepress | 2026-07-29 | MIT | `VitePress v2.0.0-alpha.18` |
| `generic` | `vuejs.org-guide-introduction-html` | https://vuejs.org/guide/introduction.html | 2026-07-29 | MIT | `VitePress v2.0.0-alpha.17` |
| `generic` | `www.postgresql.org-docs-current` | https://www.postgresql.org/docs/current/ | 2026-07-29 | PostgreSQL licence | `(none)` |
| `generic` | `www.python.org` | https://www.python.org/ | 2026-07-29 | PSF-2.0 | `(none)` |
| `generic` | `www.rust-lang.org` | https://rust-lang.org/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `generic` | `xarray.dev` | https://xarray.dev/ | 2026-07-29 | Apache-2.0 | `(none)` |
| `mdbook` | `doc.rust-lang.org-book` | https://doc.rust-lang.org/book/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `doc.rust-lang.org-cargo` | https://doc.rust-lang.org/cargo/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `doc.rust-lang.org-clippy` | https://doc.rust-lang.org/clippy/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `doc.rust-lang.org-edition-guide` | https://doc.rust-lang.org/edition-guide/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `doc.rust-lang.org-nomicon` | https://doc.rust-lang.org/nomicon/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `doc.rust-lang.org-reference` | https://doc.rust-lang.org/reference/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `doc.rust-lang.org-rust-by-example` | https://doc.rust-lang.org/rust-by-example/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `doc.rust-lang.org-rustdoc` | https://doc.rust-lang.org/rustdoc/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `doc.rust-lang.org-style-guide` | https://doc.rust-lang.org/style-guide/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `google.github.io-comprehensive-rust` | https://google.github.io/comprehensive-rust/ | 2026-07-29 | Apache-2.0 | `(none)` |
| `mdbook` | `nnethercote.github.io-perf-book` | https://nnethercote.github.io/perf-book/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `rust-cli.github.io-book` | https://rust-cli.github.io/book/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `rust-lang.github.io-api-guidelines` | https://rust-lang.github.io/api-guidelines/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `rust-lang.github.io-async-book` | https://rust-lang.github.io/async-book/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `rust-lang.github.io-mdbook` | https://rust-lang.github.io/mdBook/ | 2026-07-29 | MPL-2.0 | `(none)` |
| `mdbook` | `rust-random.github.io-book` | https://rust-random.github.io/book/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `rust-unofficial.github.io-patterns` | https://rust-unofficial.github.io/patterns/ | 2026-07-29 | MPL-2.0 | `(none)` |
| `mdbook` | `rustc-dev-guide.rust-lang.org` | https://rustc-dev-guide.rust-lang.org/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `rustwasm.github.io-docs-book` | https://rustwasm.github.io/docs/book/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mdbook` | `veykril.github.io-tlborm` | https://lukaswirth.dev/tlborm/ | 2026-07-29 | MIT OR Apache-2.0 | `(none)` |
| `mkdocs` | `docs.astral.sh-ruff` | https://docs.astral.sh/ruff/ | 2026-07-29 | MIT | `mkdocs-1.6.1, mkdocs-material-9.7.6` |
| `mkdocs` | `docs.astral.sh-uv` | https://docs.astral.sh/uv/ | 2026-07-29 | MIT OR Apache-2.0 | `mkdocs-1.6.1, mkdocs-material-9.7.6` |
| `mkdocs` | `fastapi.tiangolo.com` | https://fastapi.tiangolo.com/ | 2026-07-29 | MIT | `zensical-0.0.47` |
| `mkdocs` | `mkdocs-macros-plugin.readthedocs.io-en-latest` | https://mkdocs-macros-plugin.readthedocs.io/en/latest/ | 2026-07-29 | MIT | `(none)` |
| `mkdocs` | `mkdocstrings.github.io` | https://mkdocstrings.github.io/ | 2026-07-29 | ISC | `zensical-0.0.50` |
| `mkdocs` | `pdm-project.org-latest` | https://pdm-project.org/latest/ | 2026-07-29 | MIT | `zensical-0.0.40` |
| `mkdocs` | `sqlmodel.tiangolo.com` | https://sqlmodel.tiangolo.com/ | 2026-07-29 | MIT | `zensical-0.0.47` |
| `mkdocs` | `squidfunk.github.io-mkdocs-material` | https://squidfunk.github.io/mkdocs-material/ | 2026-07-29 | MIT | `(none)` |
| `mkdocs` | `typer.tiangolo.com` | https://typer.tiangolo.com/ | 2026-07-29 | MIT | `zensical-0.0.47` |
| `mkdocs` | `www.mkdocs.org` | https://www.mkdocs.org/ | 2026-07-29 | BSD-2-Clause | `(none)` |
| `mkdocs` | `www.python-httpx.org` | https://www.python-httpx.org/ | 2026-07-29 | BSD-3-Clause | `mkdocs-1.6.1, mkdocs-material-9.5.47` |
| `mkdocs` | `www.starlette.io` | https://www.starlette.io/ | 2026-07-29 | BSD-3-Clause | `zensical-0.0.43` |
| `rustdoc` | `doc.rust-lang.org-alloc` | https://doc.rust-lang.org/alloc/ | 2026-07-29 | MIT OR Apache-2.0 | `rustdoc` |
| `rustdoc` | `doc.rust-lang.org-core` | https://doc.rust-lang.org/core/ | 2026-07-29 | MIT OR Apache-2.0 | `rustdoc` |
| `rustdoc` | `doc.rust-lang.org-std` | https://doc.rust-lang.org/std/ | 2026-07-29 | MIT OR Apache-2.0 | `rustdoc` |
| `rustdoc` | `docs.rs-anyhow-latest-anyhow` | https://docs.rs/anyhow/latest/anyhow/ | 2026-07-29 | MIT OR Apache-2.0 | `rustdoc` |
| `rustdoc` | `docs.rs-axum-latest-axum` | https://docs.rs/axum/latest/axum/ | 2026-07-29 | MIT | `rustdoc` |
| `rustdoc` | `docs.rs-bytes-latest-bytes` | https://docs.rs/bytes/latest/bytes/ | 2026-07-29 | MIT | `rustdoc` |
| `rustdoc` | `docs.rs-chrono-latest-chrono` | https://docs.rs/chrono/latest/chrono/ | 2026-07-29 | MIT OR Apache-2.0 | `rustdoc` |
| `rustdoc` | `docs.rs-clap-latest-clap` | https://docs.rs/clap/latest/clap/ | 2026-07-29 | MIT OR Apache-2.0 | `rustdoc` |
| `rustdoc` | `docs.rs-hyper-latest-hyper` | https://docs.rs/hyper/latest/hyper/ | 2026-07-29 | MIT | `rustdoc` |
| `rustdoc` | `docs.rs-itertools-latest-itertools` | https://docs.rs/itertools/latest/itertools/ | 2026-07-29 | MIT OR Apache-2.0 | `rustdoc` |
| `rustdoc` | `docs.rs-log-latest-log` | https://docs.rs/log/latest/log/ | 2026-07-29 | MIT OR Apache-2.0 | `rustdoc` |
| `rustdoc` | `docs.rs-rayon-latest-rayon` | https://docs.rs/rayon/latest/rayon/ | 2026-07-29 | MIT OR Apache-2.0 | `rustdoc` |
| `rustdoc` | `docs.rs-regex-latest-regex` | https://docs.rs/regex/latest/regex/ | 2026-07-29 | MIT OR Apache-2.0 | `rustdoc` |
| `rustdoc` | `docs.rs-reqwest-latest-reqwest` | https://docs.rs/reqwest/latest/reqwest/ | 2026-07-29 | MIT OR Apache-2.0 | `rustdoc` |
| `rustdoc` | `docs.rs-serde-latest-serde` | https://docs.rs/serde/latest/serde/ | 2026-07-29 | MIT OR Apache-2.0 | `rustdoc` |
| `rustdoc` | `docs.rs-tantivy-latest-tantivy` | https://docs.rs/tantivy/latest/tantivy/ | 2026-07-29 | MIT | `rustdoc` |
| `rustdoc` | `docs.rs-thiserror-latest-thiserror` | https://docs.rs/thiserror/latest/thiserror/ | 2026-07-29 | MIT OR Apache-2.0 | `rustdoc` |
| `rustdoc` | `docs.rs-tokio-latest-tokio` | https://docs.rs/tokio/latest/tokio/ | 2026-07-29 | MIT | `rustdoc` |
| `rustdoc` | `docs.rs-tracing-latest-tracing` | https://docs.rs/tracing/latest/tracing/ | 2026-07-29 | MIT | `rustdoc` |
| `rustdoc` | `docs.rs-uuid-latest-uuid` | https://docs.rs/uuid/latest/uuid/ | 2026-07-29 | MIT OR Apache-2.0 | `rustdoc` |
| `sphinx` | `black.readthedocs.io-en-stable` | https://black.readthedocs.io/en/stable/ | 2026-07-29 | MIT | `(none)` |
| `sphinx` | `click.palletsprojects.com-en-stable` | https://click.palletsprojects.com/en/stable/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `coverage.readthedocs.io-en-latest` | https://coverage.readthedocs.io/en/latest/ | 2026-07-29 | Apache-2.0 | `(none)` |
| `sphinx` | `docs.aiohttp.org-en-stable` | https://docs.aiohttp.org/en/stable/ | 2026-07-29 | Apache-2.0 | `(none)` |
| `sphinx` | `docs.celeryq.dev-en-stable` | https://docs.celeryq.dev/en/stable/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `docs.dask.org-en-stable` | https://docs.dask.org/en/stable/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `docs.djangoproject.com-en-stable` | https://docs.djangoproject.com/en/6.0/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `docs.h5py.org-en-stable` | https://docs.h5py.org/en/stable/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `docs.jupyter.org-en-latest` | https://docs.jupyter.org/en/latest/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `docs.litestar.dev-latest` | https://docs.litestar.dev/latest/ | 2026-07-29 | MIT | `(none)` |
| `sphinx` | `docs.locust.io-en-stable` | https://docs.locust.io/en/stable/ | 2026-07-29 | MIT | `(none)` |
| `sphinx` | `docs.pytest.org-en-stable` | https://docs.pytest.org/en/stable/ | 2026-07-29 | MIT | `(none)` |
| `sphinx` | `docs.python.org-3` | https://docs.python.org/3/ | 2026-07-29 | PSF-2.0 | `(none)` |
| `sphinx` | `docs.readthedocs.io-en-stable` | https://docs.readthedocs.com/platform/stable/ | 2026-07-29 | MIT | `(none)` |
| `sphinx` | `docs.scipy.org-doc-scipy` | https://docs.scipy.org/doc/scipy/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `docs.scrapy.org-en-latest` | https://docs.scrapy.org/en/latest/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `docs.snapcraft.io` | https://snapcraft.io/docs/ | 2026-07-29 | CC-BY-SA-4.0 | `(none)` |
| `sphinx` | `docs.sqlalchemy.org-en-20` | https://docs.sqlalchemy.org/en/20/ | 2026-07-29 | MIT | `(none)` |
| `sphinx` | `docs.sympy.org-latest` | https://docs.sympy.org/latest/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `docs.xarray.dev-en-stable` | https://docs.xarray.dev/en/stable/ | 2026-07-29 | Apache-2.0 | `(none)` |
| `sphinx` | `flake8.pycqa.org-en-latest` | https://flake8.pycqa.org/en/latest/ | 2026-07-29 | MIT | `(none)` |
| `sphinx` | `flask.palletsprojects.com-en-stable` | https://flask.palletsprojects.com/en/stable/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `ipython.readthedocs.io-en-stable` | https://ipython.readthedocs.io/en/stable/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `jinja.palletsprojects.com-en-stable` | https://jinja.palletsprojects.com/en/stable/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `matplotlib.org-stable` | https://matplotlib.org/stable/ | 2026-07-29 | PSF-based (matplotlib) | `(none)` |
| `sphinx` | `mypy.readthedocs.io-en-stable` | https://mypy.readthedocs.io/en/stable/ | 2026-07-29 | MIT | `(none)` |
| `sphinx` | `networkx.org-documentation-stable` | https://networkx.org/documentation/stable/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `numpy.org-doc-stable` | https://numpy.org/doc/stable/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `pandas.pydata.org-docs` | https://pandas.pydata.org/docs/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `pillow.readthedocs.io-en-stable` | https://pillow.readthedocs.io/en/stable/ | 2026-07-29 | MIT-CMU | `(none)` |
| `sphinx` | `pip.pypa.io-en-stable` | https://pip.pypa.io/en/stable/ | 2026-07-29 | MIT | `(none)` |
| `sphinx` | `pygments.org-docs` | https://pygments.org/docs/ | 2026-07-29 | BSD-2-Clause | `(none)` |
| `sphinx` | `requests.readthedocs.io-en-latest` | https://requests.readthedocs.io/en/latest/ | 2026-07-29 | Apache-2.0 | `(none)` |
| `sphinx` | `scikit-learn.org-stable` | https://scikit-learn.org/stable/ | 2026-07-29 | BSD-3-Clause | `(none)` |
| `sphinx` | `setuptools.pypa.io-en-latest` | https://setuptools.pypa.io/en/latest/ | 2026-07-29 | MIT | `(none)` |
| `sphinx` | `urllib3.readthedocs.io-en-stable` | https://urllib3.readthedocs.io/en/stable/ | 2026-07-29 | MIT | `(none)` |
| `sphinx` | `virtualenv.pypa.io-en-latest` | https://virtualenv.pypa.io/en/latest/ | 2026-07-29 | MIT | `(none)` |
| `sphinx` | `www.attrs.org-en-stable` | https://www.attrs.org/en/stable/ | 2026-07-29 | MIT | `(none)` |
| `sphinx` | `www.pyinvoke.org` | https://www.pyinvoke.org/ | 2026-07-29 | BSD-2-Clause | `(none)` |
| `sphinx` | `www.sphinx-doc.org-en-master` | https://www.sphinx-doc.org/en/master/ | 2026-07-29 | BSD-2-Clause | `(none)` |
