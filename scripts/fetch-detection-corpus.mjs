#!/usr/bin/env node
//
// Fetch the platform-detection corpus (S2-9, spec P2-020).
//
//   node scripts/fetch-detection-corpus.mjs [--only <platform>] [--dry-run]
//
// A maintenance script, not part of the gate. The corpus it writes is
// committed; this exists so the capture is reproducible and its rules are
// written down rather than remembered.
//
// # The rules it enforces
//
//  1. **robots.txt is honoured**, per the same rule the crawler follows. A
//     host that disallows the path is skipped and reported, not fetched
//     anyway because it is "only one page".
//  2. **One request at a time, with a delay.** A corpus build is not a reason
//     to hammer a hundred volunteer-run documentation hosts.
//  3. **The crawler's self-identifying user agent**, so an operator reading
//     their logs can see who it was and where to complain.
//  4. **Only permissively-licensed sites are in the list below.** SPIKE-010's
//     gate: committing a fetched page is redistribution, and this repository
//     is intended to go public. Every entry carries the licence it is
//     redistributed under, and those land in SOURCES.md.
//  5. **Fixtures are truncated**, because a detector reads the head and the
//     page's furniture, not the whole document. Keeping full homepages would
//     add megabytes to the repository for bytes nothing reads.
//
// # On labelling
//
// The `platform` field below is ground truth about what built each site, and
// it is asserted by a person, not derived from the page. Where the page
// self-identifies (`<meta name="generator">`) the script cross-checks and
// reports disagreements — those are either a mislabelled entry here or a site
// that has been rebuilt, and both want a human.

import { mkdir, writeFile, rm } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { existsSync } from 'node:fs';

const ROOT = process.cwd();
const OUT = join(ROOT, 'crates/tome-core/corpus/detection/fixtures');
const USER_AGENT = 'Tome/0.0.0 (+https://github.com/alexnodeland/tome)';
const DELAY_MS = 400;

/** How much of the document to keep. */
const HEAD_LIMIT = 24_000;
const BODY_HEAD = 6_000;
const BODY_TAIL = 3_000;

// Response headers worth keeping. A detector may legitimately read these —
// `x-served-by: Read the Docs` is a real signal — and everything else is
// either noise or identifies the request rather than the site.
const KEEP_HEADERS = [
  'server',
  'content-type',
  'x-served-by',
  'x-rtd-project',
  'x-rtd-version',
  'x-backend',
  'x-powered-by',
  'x-github-request-id',
  'x-vercel-id',
  'x-amz-cf-pop',
  'via',
];

/**
 * The corpus.
 *
 * `platform` is the label the harness scores against. `licence` is what the
 * site's documentation is published under, and is the reason the fixture may
 * be committed at all.
 */
const SITES = [
  // ---------------------------------------------------------------- Sphinx
  { u: 'https://docs.python.org/3/', p: 'sphinx', l: 'PSF-2.0' },
  { u: 'https://www.sphinx-doc.org/en/master/', p: 'sphinx', l: 'BSD-2-Clause' },
  { u: 'https://numpy.org/doc/stable/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://docs.scipy.org/doc/scipy/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://pandas.pydata.org/docs/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://matplotlib.org/stable/', p: 'sphinx', l: 'PSF-based (matplotlib)' },
  { u: 'https://scikit-learn.org/stable/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://docs.djangoproject.com/en/stable/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://flask.palletsprojects.com/en/stable/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://click.palletsprojects.com/en/stable/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://jinja.palletsprojects.com/en/stable/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://requests.readthedocs.io/en/latest/', p: 'sphinx', l: 'Apache-2.0' },
  { u: 'https://docs.sqlalchemy.org/en/20/', p: 'sphinx', l: 'MIT' },
  { u: 'https://docs.pytest.org/en/stable/', p: 'sphinx', l: 'MIT' },
  { u: 'https://setuptools.pypa.io/en/latest/', p: 'sphinx', l: 'MIT' },
  { u: 'https://pip.pypa.io/en/stable/', p: 'sphinx', l: 'MIT' },
  { u: 'https://virtualenv.pypa.io/en/latest/', p: 'sphinx', l: 'MIT' },
  { u: 'https://docs.scrapy.org/en/latest/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://networkx.org/documentation/stable/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://docs.sympy.org/latest/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://docs.h5py.org/en/stable/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://urllib3.readthedocs.io/en/stable/', p: 'sphinx', l: 'MIT' },
  { u: 'https://www.attrs.org/en/stable/', p: 'sphinx', l: 'MIT' },
  { u: 'https://docs.readthedocs.io/en/stable/', p: 'sphinx', l: 'MIT' },
  { u: 'https://ipython.readthedocs.io/en/stable/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://docs.jupyter.org/en/latest/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://docs.dask.org/en/stable/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://docs.astropy.org/en/stable/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://docs.aiohttp.org/en/stable/', p: 'sphinx', l: 'Apache-2.0' },
  { u: 'https://mypy.readthedocs.io/en/stable/', p: 'sphinx', l: 'MIT' },
  { u: 'https://black.readthedocs.io/en/stable/', p: 'sphinx', l: 'MIT' },
  { u: 'https://flake8.pycqa.org/en/latest/', p: 'sphinx', l: 'MIT' },
  { u: 'https://coverage.readthedocs.io/en/latest/', p: 'sphinx', l: 'Apache-2.0' },
  { u: 'https://docs.locust.io/en/stable/', p: 'sphinx', l: 'MIT' },
  { u: 'https://www.pyinvoke.org/', p: 'sphinx', l: 'BSD-2-Clause' },
  { u: 'https://docs.celeryq.dev/en/stable/', p: 'sphinx', l: 'BSD-3-Clause' },
  { u: 'https://pygments.org/docs/', p: 'sphinx', l: 'BSD-2-Clause' },
  { u: 'https://pillow.readthedocs.io/en/stable/', p: 'sphinx', l: 'MIT-CMU' },
  { u: 'https://xarray.dev/', p: 'generic', l: 'Apache-2.0' },
  { u: 'https://docs.xarray.dev/en/stable/', p: 'sphinx', l: 'Apache-2.0' },

  // --------------------------------------------------------------- rustdoc
  { u: 'https://doc.rust-lang.org/std/', p: 'rustdoc', l: 'MIT OR Apache-2.0' },
  { u: 'https://doc.rust-lang.org/core/', p: 'rustdoc', l: 'MIT OR Apache-2.0' },
  { u: 'https://doc.rust-lang.org/alloc/', p: 'rustdoc', l: 'MIT OR Apache-2.0' },
  { u: 'https://docs.rs/serde/latest/serde/', p: 'rustdoc', l: 'MIT OR Apache-2.0' },
  { u: 'https://docs.rs/tokio/latest/tokio/', p: 'rustdoc', l: 'MIT' },
  { u: 'https://docs.rs/regex/latest/regex/', p: 'rustdoc', l: 'MIT OR Apache-2.0' },
  { u: 'https://docs.rs/clap/latest/clap/', p: 'rustdoc', l: 'MIT OR Apache-2.0' },
  { u: 'https://docs.rs/anyhow/latest/anyhow/', p: 'rustdoc', l: 'MIT OR Apache-2.0' },
  { u: 'https://docs.rs/thiserror/latest/thiserror/', p: 'rustdoc', l: 'MIT OR Apache-2.0' },
  { u: 'https://docs.rs/reqwest/latest/reqwest/', p: 'rustdoc', l: 'MIT OR Apache-2.0' },
  { u: 'https://docs.rs/tracing/latest/tracing/', p: 'rustdoc', l: 'MIT' },
  { u: 'https://docs.rs/rayon/latest/rayon/', p: 'rustdoc', l: 'MIT OR Apache-2.0' },
  { u: 'https://docs.rs/chrono/latest/chrono/', p: 'rustdoc', l: 'MIT OR Apache-2.0' },
  { u: 'https://docs.rs/tantivy/latest/tantivy/', p: 'rustdoc', l: 'MIT' },
  { u: 'https://docs.rs/hyper/latest/hyper/', p: 'rustdoc', l: 'MIT' },
  { u: 'https://docs.rs/axum/latest/axum/', p: 'rustdoc', l: 'MIT' },
  { u: 'https://docs.rs/uuid/latest/uuid/', p: 'rustdoc', l: 'MIT OR Apache-2.0' },
  { u: 'https://docs.rs/bytes/latest/bytes/', p: 'rustdoc', l: 'MIT' },
  { u: 'https://docs.rs/itertools/latest/itertools/', p: 'rustdoc', l: 'MIT OR Apache-2.0' },
  { u: 'https://docs.rs/log/latest/log/', p: 'rustdoc', l: 'MIT OR Apache-2.0' },

  // ---------------------------------------------------------------- mdBook
  { u: 'https://doc.rust-lang.org/book/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://doc.rust-lang.org/cargo/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://doc.rust-lang.org/nomicon/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://doc.rust-lang.org/reference/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://doc.rust-lang.org/rust-by-example/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://doc.rust-lang.org/edition-guide/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://doc.rust-lang.org/rustdoc/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://doc.rust-lang.org/style-guide/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://doc.rust-lang.org/clippy/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://rust-lang.github.io/mdBook/', p: 'mdbook', l: 'MPL-2.0' },
  { u: 'https://rust-lang.github.io/api-guidelines/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://rust-lang.github.io/async-book/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://google.github.io/comprehensive-rust/', p: 'mdbook', l: 'Apache-2.0' },
  { u: 'https://rust-cli.github.io/book/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://rust-random.github.io/book/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://rustwasm.github.io/docs/book/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://nnethercote.github.io/perf-book/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://veykril.github.io/tlborm/', p: 'mdbook', l: 'MIT OR Apache-2.0' },
  { u: 'https://rust-unofficial.github.io/patterns/', p: 'mdbook', l: 'MPL-2.0' },
  { u: 'https://rustc-dev-guide.rust-lang.org/', p: 'mdbook', l: 'MIT OR Apache-2.0' },

  // ---------------------------------------------------------------- MkDocs
  { u: 'https://www.mkdocs.org/', p: 'mkdocs', l: 'BSD-2-Clause' },
  { u: 'https://squidfunk.github.io/mkdocs-material/', p: 'mkdocs', l: 'MIT' },
  { u: 'https://fastapi.tiangolo.com/', p: 'mkdocs', l: 'MIT' },
  { u: 'https://typer.tiangolo.com/', p: 'mkdocs', l: 'MIT' },
  { u: 'https://sqlmodel.tiangolo.com/', p: 'mkdocs', l: 'MIT' },
  // Migrated to Astro: no Material markup at all, so no MkDocs scraper
  // would handle it. Ground truth changed under the label.
  { u: 'https://docs.pydantic.dev/latest/', p: 'generic', l: 'MIT' },
  { u: 'https://www.uvicorn.org/', p: 'mkdocs', l: 'BSD-3-Clause' },
  { u: 'https://www.starlette.io/', p: 'mkdocs', l: 'BSD-3-Clause' },
  { u: 'https://www.python-httpx.org/', p: 'mkdocs', l: 'BSD-3-Clause' },
  { u: 'https://mkdocstrings.github.io/', p: 'mkdocs', l: 'ISC' },
  { u: 'https://pdm-project.org/latest/', p: 'mkdocs', l: 'MIT' },
  { u: 'https://python-poetry.org/docs/', p: 'generic', l: 'MIT' },
  // On readthedocs.io and built with **MkDocs**, not Sphinx. RTD hosts both;
  // the domain says who serves the pages, not what made them.
  { u: 'https://mkdocs-macros-plugin.readthedocs.io/en/latest/', p: 'mkdocs', l: 'MIT' },
  { u: 'https://docs.litestar.dev/latest/', p: 'sphinx', l: 'MIT' },
  { u: 'https://docs.astral.sh/ruff/', p: 'mkdocs', l: 'MIT' },
  { u: 'https://docs.astral.sh/uv/', p: 'mkdocs', l: 'MIT OR Apache-2.0' },

  // ------------------------------------------------------------ Docusaurus
  { u: 'https://docusaurus.io/docs', p: 'docusaurus', l: 'MIT' },
  { u: 'https://jestjs.io/docs/getting-started', p: 'docusaurus', l: 'MIT' },
  { u: 'https://reactnative.dev/docs/getting-started', p: 'docusaurus', l: 'MIT' },
  { u: 'https://redux.js.org/', p: 'docusaurus', l: 'MIT' },
  { u: 'https://prettier.io/docs/', p: 'docusaurus', l: 'MIT' },
  { u: 'https://babeljs.io/docs/', p: 'docusaurus', l: 'MIT' },
  { u: 'https://create-react-app.dev/docs/getting-started', p: 'docusaurus', l: 'MIT' },
  { u: 'https://www.electronjs.org/docs/latest', p: 'docusaurus', l: 'MIT' },
  // Migrated to Next.js.
  { u: 'https://graphql.org/learn/', p: 'generic', l: 'MIT' },
  { u: 'https://socket.io/docs/v4/', p: 'docusaurus', l: 'MIT' },
  { u: 'https://typescript-eslint.io/', p: 'docusaurus', l: 'MIT' },
  // Migrated to Rspress. One stray 'docusaurus' string survives in the
  // page, which is exactly the sort of thing a naive detector trips on.
  { u: 'https://swc.rs/docs/getting-started', p: 'generic', l: 'Apache-2.0' },
  // Dropped: the archived Yarn v1 site was Docusaurus v1, whose markers differ
  // from v2's, and the page now serves nothing that establishes either. A
  // fixture whose ground truth cannot be asserted is worse than no fixture.
  { u: 'https://docs.snapcraft.io/', p: 'sphinx', l: 'CC-BY-SA-4.0' },
  { u: 'https://reactrouter.com/', p: 'generic', l: 'MIT' },
  { u: 'https://tsup.egoist.dev/', p: 'generic', l: 'MIT' },

  // --------------------------------------------------------------- GitBook
  //
  // **Deliberately empty.** GitBook is now a hosted product, and the public
  // instances of it are companies' own documentation — all rights reserved by
  // default. SPIKE-010's rule is that pages with unknown or per-project
  // licences stay local and uncommitted, and a corpus is not a reason to bend
  // it. `Platform::GitBook` therefore has no fixtures and the harness reports
  // an empty row rather than a score, which is the honest thing: nothing here
  // measures GitBook detection.
  //
  // A probe of ten candidate sites (Uniswap, Chainlink, WalletConnect,
  // MetaMask, Truffle, Zapier…) found that all but GitBook's own docs have
  // migrated to Docusaurus, so this is a shrinking class rather than a gap in
  // the search for one. Worth knowing before building a GitBook scraper: none
  // of P2-010..013 is one.

  // --------------------------------------------------------------- Generic
  // Documentation built by something else, plus sites that are not
  // documentation at all. P2-020 asks specifically for "a plain marketing
  // site that must classify as Generic".
  { u: 'https://www.rust-lang.org/', p: 'generic', l: 'MIT OR Apache-2.0' },
  { u: 'https://www.python.org/', p: 'generic', l: 'PSF-2.0' },
  { u: 'https://nodejs.org/en', p: 'generic', l: 'MIT' },
  { u: 'https://go.dev/doc/', p: 'generic', l: 'BSD-3-Clause' },
  { u: 'https://kubernetes.io/docs/home/', p: 'generic', l: 'CC-BY-4.0' },
  { u: 'https://gohugo.io/documentation/', p: 'generic', l: 'Apache-2.0' },
  { u: 'https://jekyllrb.com/docs/', p: 'generic', l: 'MIT' },
  { u: 'https://vuejs.org/guide/introduction.html', p: 'generic', l: 'MIT' },
  { u: 'https://vitejs.dev/guide/', p: 'generic', l: 'MIT' },
  { u: 'https://vitepress.dev/guide/what-is-vitepress', p: 'generic', l: 'MIT' },
  { u: 'https://svelte.dev/docs/svelte/overview', p: 'generic', l: 'MIT' },
  { u: 'https://tauri.app/start/', p: 'generic', l: 'MIT OR Apache-2.0' },
  { u: 'https://eslint.org/docs/latest/', p: 'generic', l: 'MIT' },
  // The GNU manuals are GFDL, whose invariant-section conditions are more
  // bookkeeping than one fixture is worth. Dropped rather than reasoned about.
  { u: 'https://curl.se/docs/', p: 'generic', l: 'curl licence (MIT-like)' },
  { u: 'https://sqlite.org/docs.html', p: 'generic', l: 'Public domain' },
  { u: 'https://www.postgresql.org/docs/current/', p: 'generic', l: 'PostgreSQL licence' },
  { u: 'https://redis.io/docs/latest/', p: 'generic', l: 'CC-BY-SA-4.0' },
  { u: 'https://caddyserver.com/docs/', p: 'generic', l: 'Apache-2.0' },
  { u: 'https://example.com/', p: 'generic', l: 'IANA reserved example domain' },
];

// ---------------------------------------------------------------- fetching

const robotsCache = new Map();

async function get(url, accept) {
  const response = await fetch(url, {
    headers: { 'user-agent': USER_AGENT, accept },
    redirect: 'follow',
  });
  return response;
}

/**
 * Whether robots.txt allows this path for our user agent.
 *
 * A deliberately conservative reading: it takes the `*` group and any group
 * naming Tome, and honours `Disallow` prefixes. It does not implement
 * `Allow` precedence in full — where the two conflict it refuses, because the
 * cost of being wrong here is fetching something an operator asked us not to.
 */
async function allowed(url) {
  const { origin, pathname } = new URL(url);
  if (!robotsCache.has(origin)) {
    let rules = [];
    try {
      const response = await get(`${origin}/robots.txt`, 'text/plain');
      if (response.ok) {
        const text = await response.text();
        let applies = false;
        for (const raw of text.split('\n')) {
          const line = raw.split('#')[0].trim();
          const [field, ...rest] = line.split(':');
          const value = rest.join(':').trim();
          if (/^user-agent$/i.test(field)) {
            applies = value === '*' || /tome/i.test(value);
          } else if (applies && /^disallow$/i.test(field) && value.length > 0) {
            rules.push(value);
          }
        }
      }
    } catch {
      // No robots.txt, or it could not be read. Absent means allowed.
      rules = [];
    }
    robotsCache.set(origin, rules);
  }
  return !robotsCache.get(origin).some((rule) => pathname.startsWith(rule));
}

/** Keep the head, the top of the body, and the foot of the body. */
function truncate(html) {
  const headEnd = html.toLowerCase().indexOf('</head>');
  const head = headEnd === -1 ? html.slice(0, HEAD_LIMIT) : html.slice(0, headEnd + 7);
  const body = headEnd === -1 ? '' : html.slice(headEnd + 7);

  if (body.length <= BODY_HEAD + BODY_TAIL) return head + body;
  // The foot matters: "Built with MkDocs", "Made with Docusaurus" and
  // mdBook's script tags all live near the end of the document, and a
  // head-only fixture would throw away the signal the detector most needs.
  return `${head}${body.slice(0, BODY_HEAD)}\n<!-- tome: ${
    body.length - BODY_HEAD - BODY_TAIL
  } bytes elided -->\n${body.slice(-BODY_TAIL)}`;
}

/** The site's own claim about what built it, for cross-checking the label. */
function generatorMeta(html) {
  const match = html.match(/<meta[^>]+name=["']generator["'][^>]*content=["']([^"']*)["']/i);
  return match ? match[1] : null;
}

function slug(url) {
  const { hostname, pathname } = new URL(url);
  const path = pathname
    .replace(/\/+$/, '')
    .replace(/^\/+/, '')
    .replace(/[^a-z0-9]+/gi, '-');
  return `${hostname}${path ? `-${path}` : ''}`.replace(/-+/g, '-').toLowerCase().slice(0, 90);
}

async function main() {
  const only = process.argv.includes('--only')
    ? process.argv[process.argv.indexOf('--only') + 1]
    : null;
  const dryRun = process.argv.includes('--dry-run');

  const targets = only ? SITES.filter((s) => s.p === only) : SITES;
  console.log(`${targets.length} sites${only ? ` (${only})` : ''}${dryRun ? ' [dry run]' : ''}\n`);

  if (!dryRun && !only && existsSync(OUT)) await rm(OUT, { recursive: true });

  const written = [];
  const skipped = [];
  const disagreed = [];

  for (const site of targets) {
    const name = slug(site.u);
    try {
      if (!(await allowed(site.u))) {
        skipped.push(`${site.u} — robots.txt disallows it`);
        continue;
      }

      const response = await get(site.u, 'text/html');
      if (!response.ok) {
        skipped.push(`${site.u} — HTTP ${response.status}`);
        continue;
      }
      const html = await response.text();

      const generator = generatorMeta(html);
      if (generator) {
        const claim = generator.toLowerCase();
        const label = site.p;
        const consistent =
          (label === 'sphinx' && claim.includes('sphinx')) ||
          (label === 'rustdoc' && claim.includes('rustdoc')) ||
          (label === 'mdbook' && claim.includes('mdbook')) ||
          (label === 'mkdocs' && claim.includes('mkdocs')) ||
          (label === 'docusaurus' && claim.includes('docusaurus')) ||
          (label === 'gitbook' && claim.includes('gitbook')) ||
          label === 'generic';
        if (!consistent) {
          disagreed.push(`${site.u} — labelled ${label}, page says "${generator}"`);
        }
      }

      const headers = KEEP_HEADERS.filter((h) => response.headers.get(h) !== null)
        .map((h) => `header: ${h}: ${response.headers.get(h)}`)
        .join('\n');

      const fixture = [
        '# tome-detection-fixture v1',
        `url: ${response.url}`,
        `captured: ${new Date().toISOString().slice(0, 10)}`,
        `licence: ${site.l}`,
        `platform: ${site.p}`,
        `generator-meta: ${generator ?? '(none)'}`,
        'notes: head kept in full; body truncated (see fetch-detection-corpus.mjs)',
        headers,
        '---',
        truncate(html),
      ]
        .filter((line) => line.length > 0)
        .join('\n');

      const file = join(OUT, site.p, `${name}.fixture`);
      if (!dryRun) {
        await mkdir(dirname(file), { recursive: true });
        await writeFile(file, fixture);
      }
      written.push({ ...site, name, generator, bytes: fixture.length });
      process.stdout.write('.');
    } catch (error) {
      skipped.push(`${site.u} — ${error.message}`);
      process.stdout.write('!');
    }
    await new Promise((resolve) => setTimeout(resolve, DELAY_MS));
  }

  console.log(`\n\nwrote ${written.length}, skipped ${skipped.length}`);
  const counts = {};
  for (const site of written) counts[site.p] = (counts[site.p] ?? 0) + 1;
  console.log('by platform:', counts);
  console.log('total size:', `${Math.round(written.reduce((n, s) => n + s.bytes, 0) / 1024)} KB`);

  if (disagreed.length > 0) {
    console.log('\nLABEL DISAGREEMENTS — these want a human:');
    for (const line of disagreed) console.log(`  ${line}`);
  }
  if (skipped.length > 0) {
    console.log('\nskipped:');
    for (const line of skipped) console.log(`  ${line}`);
  }

  if (!dryRun) {
    await writeFile(join(OUT, '..', 'MANIFEST.json'), `${JSON.stringify(written, null, 2)}\n`);
  }
}

await main();
