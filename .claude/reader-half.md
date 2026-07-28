# Continuation note — Tome, the reader half (S1-11..S1-15)

**Written:** 2026-07-28. Pick this up to build the reader. The ingestion backend (S1-1..S1-10) is
done and merged; this note is only about what turns the pipeline's output into something a person
sees. When the reader renders `docs.python.org` offline, Stage 1 is done — delete this file and
`.claude/continuation.md`.

**First thing to run:**
```bash
cd /Users/alexandernodeland/anodeland/projects/code/apps/tome
git checkout main && git pull
./scripts/check.sh --fast     # ~60s, should print "All checks passed."
```
`scripts/check.sh` is the gate (CI can't run — private repo + Actions blocked). Change it and
`.github/workflows/ci.yml` together. Merge cadence this project uses: one PR per ticket, full gate
green, merge to `main`, continue. **Do not stack PRs** — merging a stacked parent with
`--delete-branch` closes the child unrecoverably (learned the hard way; see `.claude/continuation.md`).

---

## What already exists (so you don't rebuild it)

**The backend pipeline is complete**, all in `crates/tome-core`, all pure functions tested against
the fixture server. Given a source config, the flow is:

```
config::SourceConfig::parse_file           →  validated config
crawl::Crawler::new(&fetcher, &config)
      .crawl(&mut on_progress)             →  crawl::CrawlOutcome { docset: DocSet, errors, hit_page_cap }
    (Fetcher already does robots + rate-limit + SSRF; UrlFilter does scope)
per page (DocPage { meta: Page, body: Node }):
    normalize::normalize(body, &base)      →  NormalizedPage { title, description, body: Node }
    sanitize::sanitize(node)               →  Node   (XSS-safe; anchors intact)
    assets::localize_assets(node,&base,&mut store) → LocalizeOutcome { body, errors }
```

The output the reader consumes is a **`model::Node`** tree (a frozen, serde-stable AST — see
`crates/tome-core/src/model/node.rs`). Node kinds: `Document, Heading{level,id,children},
Paragraph, CodeBlock{language,code}, Blockquote, List{ordered,start,items}, DefinitionList{items},
Table{headers,rows}, Admonition{kind,title,children}, Image{src,alt}, ThematicBreak, Text{value},
Emphasis, Strong, InlineCode{code}, Link{href,title,children}, Anchor{id}, LineBreak`. There is
**no raw-HTML node, on purpose** — that's the first XSS wall.

**Persistence** (`db.rs`): `Database::open(&paths)`, `upsert_source`, `upsert_page`, `get_page`,
`list_pages`. Page *metadata* is in SQLite; page *content* (the AST / rendered HTML) is meant to
live on disk under `paths.pages_dir(&source_id)` — not written by any stage yet; wiring that is
part of making the reader load real pages.

**The app shell** (Tauri): `src-tauri/src/lib.rs` has one command, `library_location`. The frontend
(`src/`) is the scaffold — `App.svelte` shows the library path, `src/lib/tauri.ts` is the single
`invoke` seam, `src/main.ts` boots it (and has a `TOME_SPIKE_002` hook that runs the spike harness
instead — that harness, `src-tauri/src/spike002.rs` + `src/spike/spike002.ts` +
`public/spike002-frame.js`, gets **deleted when S1-13 lands**).

**Verify differently now.** The backend is `cargo test`. The reader needs a *running app* —
`npm run tauri dev` (or `dev` + the built app). `cargo test` can cover the AST→HTML renderer
(pure), but layout/typography/iframe behaviour need eyes on the window. SPIKE-002 also showed
`screencapture` is unreliable here (it caught an unrelated window once) — prefer functional checks
(assert on DOM via the browser tools, or console-log probes) over screenshots.

---

## The one architecture decision to make first (S1-11 shape)

**Rendering is AST → HTML, done in Rust, and it is where highlighting and escaping live.** The AST
is the semantic model; HTML is presentation. So:

- Build an **`render` module in `tome-core`** (or in the app) that walks a `Node` and emits HTML.
  This is the natural home for S1-11 (highlighting) and the S1-9 escaping contract (below).
- **Syntax highlighting is a render concern, not an AST mutation.** `CodeBlock` stays
  `{language, code}`; the renderer highlights `code` when it emits the `<pre><code>`. Recommended:
  **syntect with `ClassedHTMLGenerator`** → CSS-class spans (not inline styles), so light/dark is a
  theme swap in S1-12's CSS with no re-highlighting, and it works under the strict CSP with no
  client JS. Trade-off to confirm with the owner: **syntect is a heavy dependency** (bundled
  syntax + theme dumps, adds build weight). A lighter highlighter (or none, plain `<code>`) is the
  alternative. This was flagged for owner input and is still open.

### The renderer's non-negotiable contract (S1-9 depends on it)

The sanitizer deliberately does **not** charset-strip free-text fields (`Link.title`, `Image.alt`,
`Admonition.title`, and all `Text`/`InlineCode`/`CodeBlock.code`). Their safety is the renderer's
job, and it is a hard contract the refute-panel made explicit:

> **The renderer MUST quote every attribute and HTML-escape every attribute value and every text
> node** (`& < > "` at minimum, in the right context). Ids/schemes/class-tokens are already
> sanitized to safe charsets; free text is not, so escaping is what makes it safe.

If you skip this, you reintroduce XSS that S1-9's tests assume is handled downstream. Put an
escaping helper at the base of the renderer and route every string through it.

---

## The reader inherits these from SPIKE-002 (don't rediscover them)

Full write-up: `docs/spikes/002-reader-iframe-bridge.md`. The measured facts S1-13 is built on:

- The reader is a **sandboxed `<iframe>`** (`sandbox="allow-scripts"`, **no** `allow-same-origin` →
  opaque origin) inside Tauri's primary webview. `__TAURI_INTERNALS__` is **unreachable** from the
  frame (verified) — untrusted page HTML cannot reach the IPC layer.
- **One `postMessage` carries a full 500 KB page in ~0.1 ms.** Do not build a streaming/chunking
  protocol — chunking measured *slower*. Protocol: one `{type:"page", html}` message.
- The `srcdoc` frame **inherits the app CSP** and a `<meta>` CSP stacks on top (both enforce).
  The frame's bootstrap script **cannot be inline** (no `unsafe-inline`) — serve it from the app
  origin. `'self'` is meaningless in the opaque frame; name the app origin explicitly.
- **`window.getSelection()` works** in the sandbox for quote + prefix/suffix anchoring (annotations,
  later). Ranges across element boundaries are fine.
- **Tauri events are deny-by-default**; `src-tauri/capabilities/default.json` already grants
  `core:event:default`. Add permissions there deliberately if you use more of the Tauri API.
- **Occluded windows suspend rAF and clamp timers (~230 ms)** — never gate reader correctness on
  rAF; frame-pacing under load is S1-13's *interactive* acceptance item (SPIKE-002 couldn't measure
  it headlessly).

The current app CSP is in `src-tauri/tauri.conf.json`:
`default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: data:; …`
— note `img-src 'self' asset: data:`: localized assets (S1-10 wrote them under the source's
`assets/` dir as `assets/<sha256>.<ext>`) need to be served to the frame via Tauri's `asset:`
protocol or a custom protocol; wiring that so the frame's `img` src resolves offline is part of
S1-13. The offline gate asserts **no `http` reference** survives render — S1-10 already guarantees
the AST carries none; the renderer + asset serving must keep it that way.

---

## The remaining tickets

Spec lives in `docs/plans/01-phase-1-foundation.md` (P1-014/015/016/017/018/019/020) and the
routing/models are in `docs/plans/18-implementation-plan.md` § Stage 1 (rows S1-11..S1-15).

- **S1-11 — syntax highlighting** (Fable). Standalone `highlight(code, language) -> classed HTML`
  per the decision above; graceful plain-text fallback for unknown/absent language (the sanitizer
  already guarantees `language` is a safe `[a-z0-9-]` token or `None`). Fuzz/verify: no input
  panics; output is well-formed. Called by the renderer.
- **S1-12 — typography + design tokens** (Fable). CSS variables per `docs/plans/15-design-system.md`
  and P1-015: New York serif body (SF Pro fallback), SF Mono code, SF Pro Display headings, 17px
  body / 15px code, line-height 1.6, 70ch measure, 1em paragraph spacing, **light + dark schemes**
  as CSS vars, print stylesheet. This is the CSS the highlighter's classes and the reader HTML use.
  Frontend (`src/` + the frame's stylesheet).
- **S1-13 — reader iframe + IPC bridge** (**Opus, the big one**). The AST→HTML renderer + escaping
  contract + the sandboxed-iframe bridge from SPIKE-002 + asset serving + the page-load path
  (read AST from disk/db → render → postMessage into the frame). Deletes the spike002 harness.
  Interactive acceptance: 60 Hz scroll/selection over the bridge without frame drops.
- **S1-14 — three-panel layout, library sidebar, TOC** (Fable). P1-017/018/019. The TOC comes from
  the `DocSet.toc` / heading `id`s the pipeline already produces; deep links use the anchors S1-9
  preserved.
- **S1-15 — navigation + history** (Fable). P1-020. In-app link handling, back/forward, `#fragment`
  scroll (the fragments and ids are consistent — S1-9 made sure).

**Stage 1 exit gate** (`docs/plans/18` § Stage 1): the app renders `docs.python.org` with the
network off, images included, anchors working, and the golden corpus committed. The pipeline half
is done and offline-proven; the reader makes it visible. (Owner chose Sphinx/`docs.python.org` as
the first target — the pipeline's `<dl>` API-entry handling and the seeded golden corpus are
Sphinx-first.)

---

## Standing decisions (owner, 2026-07-28) — still in force

- **Refute-panel is the standing method for security-critical work** — run the Workflow
  automatically, no re-ask. (No security tickets remain in the reader half, but annotations/auth
  later qualify.)
- **Keep merging green PRs ticket-by-ticket.**
- **Real golden-corpus expansion is deferred until the reader renders** — then validate real pages
  (Python PSF-2.0, Go/K8s CC-BY, Rust/Node MIT/Apache; all cleared by SPIKE-010) *visually*, not as
  JSON diffs. `corpus/normalization/input/SOURCES.md` has the gate; the suite is seeded with the
  repo's own fixture.
- **Syntax-highlighting dependency (syntect vs lighter)** — flagged for owner, still open; default
  is syntect + `ClassedHTMLGenerator` unless told otherwise.

## Open, non-blocking

- **PR #2** stale scaffolding branch — left open, ask before closing.
- **Dependabot #5–#13** — nine PRs, several dev-dep majors (TypeScript 7, eslint-plugin-svelte 3,
  prettier-plugin-svelte 4, jest-dom 7); Actions bumps moot until CI runs. Untouched; batch the
  majors through `check.sh` deliberately. **Worth doing before the frontend work** since some touch
  the Svelte/lint toolchain the reader UI uses.
- **DEC-005..008** (docset import, watch, note format, export) — owner's calls, non-blocking.

## Reader-relevant traps (from the full note + SPIKE-002)

- **Svelte 5 + Vitest** resolves the server build; `mount()` throws `lifecycle_function_unavailable`
  unless `conditions: ['browser']` is scoped to `process.env.VITEST` (already done in `vite.config.ts`).
- **`$lib` needs two aliases**: `paths` in `tsconfig.json` *and* `resolve.alias` in `vite.config.ts`.
- **Prettier/ESLint ignore `crates/*/fixtures/` and `crates/*/corpus/`** — golden/fixture files must
  not be reformatted. The frame bootstrap (`public/*.js`) has its own ESLint globs.
- Workspace Rust lints **deny `unwrap`/`expect`/`panic`**; test files carry a file-level
  `#![allow(...)]`. The renderer is library code — no unwraps on page content.
- `npm run lint` = eslint + prettier; `npm run check` = svelte-check; `npm run test` = vitest.
