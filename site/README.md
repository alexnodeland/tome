# The documentation site

The user-facing half of Tome's documentation. `docs/` is for people building
Tome; this is for people using it.

```sh
node site/build.mjs                     # → site/_build/
cd site/_build && python3 -m http.server 8777
```

`./scripts/check.sh` runs the build, so a broken build script fails the gate
rather than the deploy.

## Layout

| | |
|---|---|
| `pages/*.html` | Content, as HTML fragments |
| `assets/site.css` | The stylesheet. **No palette of its own** — it consumes `public/tokens.css` |
| `build.mjs` | Wraps fragments in the shell, generates the catalogue, copies the registry |

The catalogue page is **generated from `registry/index.yaml`**, not written. A
source added to the registry appears on the site with no second edit, and the
two cannot disagree.

## Why no static site generator, and no markdown

Two reasons that are the same reason:

- A markdown renderer is a dependency added to read files this project writes —
  the argument `scripts/check-contrast.mjs` makes about CSS parsers.
- The site's one distinctive device is **marginalia**: a claim paired with the
  measurement that backs it. Markdown cannot express that, so content authored
  in markdown would escape into HTML for every claim on the page — which is
  most of them.

## Design

It uses Tome's own design tokens, copied in at build time rather than
duplicated. The site is a page of the product, not a brochure beside it, and it
cannot drift from the app's palette because it has no palette of its own.
`scripts/check-contrast.mjs` checks `site/assets/site.css` for the same reason
it checks `public/reader.css`: an unresolved `var()` silently falls back rather
than erroring, and no bundler ever touches this file.

Two inheritances worth naming:

- **The body face is a serif** (`--font-body`). Tome is a reading application;
  a site selling one in the same system sans as every other developer tool is
  arguing against itself.
- **The measure is `--measure`**, the reader's own. If documentation reads well
  at that width in the app, it reads well here.

The marginalia are the one place boldness is spent. Everything else stays
quiet: one animation, on load, on the hero only.

## Claims

Every falsifiable claim on the site carries its measurement and a link to where
it was measured. **If you change a number here, change it because the harness
said so** — the citations are the site's whole argument, and a number nobody
can check is worse than no number.

## Deployment

`.github/workflows/pages.yml`, which also publishes `registry/` so the registry
is served from Pages as the PRD specifies.

> **It has never run.** GitHub Actions is blocked at the account level and the
> repository is private, so every workflow fails in seconds without executing a
> step. The workflow is what will publish the site the day that changes; until
> then, build locally.
