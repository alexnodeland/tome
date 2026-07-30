#!/usr/bin/env node
//
// Build the documentation site into `site/_build/`.
//
// **No dependencies, and no markdown.** Two reasons, both the same reason:
//
//   * A markdown renderer is a dependency added to read files this project
//     writes — the argument `scripts/check-contrast.mjs` makes about CSS
//     parsers. The pages are HTML fragments; the shell is a template literal.
//   * The site's one distinctive device is marginalia — a claim paired with
//     the measurement that backs it — and markdown has no way to express
//     that. Content authored in markdown would have to escape into HTML for
//     every claim on the page, which is most of them.
//
// What it does: wraps each fragment in the shared shell, copies the design
// tokens from `public/tokens.css` (copied, never duplicated — the site is a
// page of the product), generates the catalogue page from the real
// `registry/index.yaml`, and copies the registry itself so Pages serves it.
// The PRD says the registry is served from the project's GitHub Pages; this
// is the half that makes that true.

import { readFileSync, writeFileSync, mkdirSync, rmSync, readdirSync, cpSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const site = dirname(fileURLToPath(import.meta.url));
const root = join(site, '..');
const out = join(site, '_build');

/** Pages, in navigation order. `file` is the fragment; `path` the built URL. */
const PAGES = [
  { file: 'index.html', path: '/', title: null, nav: 'Overview' },
  { file: 'guide.html', path: '/guide/', title: 'Getting started', nav: 'Guide' },
  { file: 'cli.html', path: '/cli/', title: 'Command reference', nav: 'CLI' },
  { file: 'agents.html', path: '/agents/', title: 'Agent access', nav: 'Agents' },
  { file: 'catalogue.html', path: '/catalogue/', title: 'Catalogue', nav: 'Catalogue' },
];

const REPO = 'https://github.com/alexnodeland/tome';

/** Escape text destined for HTML. Registry values are data, not markup. */
const esc = (text) =>
  String(text)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');

/**
 * The registry index, parsed with just enough YAML to read a list of flat
 * maps — which is all `registry/index.yaml` is, and which the schema test in
 * `crates/tome-core/tests/registry.rs` holds to that shape.
 *
 * A YAML library here would be a dependency for one file whose form is
 * already pinned by a test. If the index ever grows nesting, that test is
 * where it would be declared, and this is where it would need to change.
 */
function readRegistry() {
  const text = readFileSync(join(root, 'registry/index.yaml'), 'utf8');
  const entries = [];
  let current = null;
  for (const raw of text.split('\n')) {
    const line = raw.replace(/\s+$/, '');
    if (!line.trim() || line.trim().startsWith('#')) continue;
    const item = line.match(/^\s+-\s+(\w+):\s*(.*)$/);
    if (item) {
      if (current) entries.push(current);
      current = { [item[1]]: item[2] };
      continue;
    }
    const field = line.match(/^\s+(\w+):\s*(.*)$/);
    if (field && current) current[field[1]] = field[2];
  }
  if (current) entries.push(current);
  return entries.filter((entry) => entry.id);
}

/** The shared shell. */
function shell({ title, path, body }) {
  const nav = PAGES.map((page) => {
    const current = page.path === path ? ' aria-current="page"' : '';
    return `<a href="${page.path}"${current}>${page.nav}</a>`;
  }).join('\n          ');

  const heading = title
    ? `Tome — ${title}`
    : 'Tome — a personal library for technical documentation';

  return `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${esc(heading)}</title>
    <meta
      name="description"
      content="Ingest any documentation site, read it offline with good typography, search across everything, and expose the library to coding agents over MCP. macOS, local-first, no telemetry."
    />
    <meta name="color-scheme" content="light dark" />
    <link rel="stylesheet" href="/assets/tokens.css" />
    <link rel="stylesheet" href="/assets/site.css" />
  </head>
  <body>
    <header class="masthead">
      <div class="wrap masthead-inner">
        <a class="wordmark" href="/">Tome</a>
        <nav class="masthead-nav" aria-label="Sections">
          ${nav}
        </nav>
      </div>
    </header>

    <main class="wrap">
${body}
    </main>

    <footer class="footer">
      <div class="wrap">
        <p>
          Tome is dual-licensed <a href="${REPO}/blob/main/LICENSE-MIT">MIT</a> or
          <a href="${REPO}/blob/main/LICENSE-APACHE">Apache-2.0</a>. It fetches documentation other
          people wrote, obeys <code>robots.txt</code>, and redistributes configuration — never
          content.
        </p>
        <p>
          <a href="${REPO}">Source</a> ·
          <a href="${REPO}/blob/main/docs/PRD.md">Requirements</a> ·
          <a href="${REPO}/blob/main/docs/spikes/">Measurements</a> ·
          <a href="/registry/index.yaml">registry/index.yaml</a>
        </p>
        <p>No telemetry, of any kind, including opt-in. There is nothing here that counts you.</p>
      </div>
    </footer>
  </body>
</html>
`;
}

/** The catalogue page, generated from the registry rather than written. */
function catalogueBody(sources) {
  const cards = sources
    .map((source) => {
      const verified =
        source.verified && source.verified !== 'null'
          ? `<span class="verified">verified ${esc(source.verified)}</span>`
          : `<span class="unverified">not yet verified</span>`;
      return `        <article class="card">
          <h3>${esc(source.name)}</h3>
          <p class="card-meta">
            <code>${esc(source.id)}</code> · ${esc(source.category)} · ${esc(source.licence)}<br />
            ${verified} ·
            <a href="${esc(source.homepage)}">upstream</a>
          </p>
        </article>`;
    })
    .join('\n');

  return `      <div class="prose">
        <h1>Catalogue</h1>
        <p class="lede">
          Ready-made source configurations, tested against the live sites. Copy one into your
          sources directory and run <code>tome pull</code>.
        </p>

        <div class="annotated">
          <p>
            The registry ships <strong>configuration, never content</strong>. Tome does not host or
            redistribute anyone's documentation — your machine fetches it from the origin, the same
            pages your browser would fetch, kept so they work offline.
          </p>
          <aside class="note">
            <span class="measure">${sources.length} sources</span>
            v1.0 target is 30 ·
            <a href="${REPO}/blob/main/registry/README.md">how to add one</a>
          </aside>
        </div>
      </div>

      <div class="prose">
        <div class="catalogue">
${cards}
        </div>

        <h2>What “verified” means</h2>
        <div class="annotated">
          <p>
            A scheduled job pulls 25 pages from each source's live site and asks one question: does
            this scraper still find anything? Zero pages from a site that answered is what scraper
            rot looks like. The date is when that last passed — a stale one is the signal.
          </p>
          <aside class="note">
            <span class="measure">RISK-003</span>
            scraper rot, otherwise discovered by users
          </aside>
        </div>
        <div class="annotated">
          <p>
            It caught something real on its first run. <code>nodejs.org/docs/</code> is
            <code>Disallow</code>ed by that site's <code>robots.txt</code> while
            <code>nodejs.org/api/</code> is explicitly <code>Allow</code>ed — so the obvious URL was
            the forbidden one, and the correct one was a path segment away. No review would have
            caught that; a fetch did.
          </p>
          <aside class="note">
            <span class="measure">robots.txt</span>
            obeyed by default, and
            <strong>not overridable</strong> for registry configurations
          </aside>
        </div>
      </div>
`;
}

// ---------------------------------------------------------------------------

rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, 'assets'), { recursive: true });

// Tokens are COPIED from the app, not duplicated here. The site cannot drift
// from the product's palette because it has no palette of its own.
cpSync(join(root, 'public/tokens.css'), join(out, 'assets/tokens.css'));
cpSync(join(site, 'assets/site.css'), join(out, 'assets/site.css'));

// The registry, served from Pages as the PRD specifies.
cpSync(join(root, 'registry'), join(out, 'registry'), { recursive: true });

const sources = readRegistry();

for (const page of PAGES) {
  const body =
    page.file === 'catalogue.html'
      ? catalogueBody(sources)
      : readFileSync(join(site, 'pages', page.file), 'utf8');

  const html = shell({ title: page.title, path: page.path, body });
  const dir = page.path === '/' ? out : join(out, page.path);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'index.html'), html);
}

// GitHub Pages runs Jekyll over the output unless told not to, and Jekyll
// ignores files and directories beginning with an underscore. Nothing here
// starts with one today, but a build artifact that silently vanishes is a
// bad way to find that out.
writeFileSync(join(out, '.nojekyll'), '');

const built = readdirSync(out).length;
console.log(
  `site: ${PAGES.length} pages, ${sources.length} catalogue entries, ${built} top-level entries → site/_build`,
);
