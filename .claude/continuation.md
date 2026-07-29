# Continuation note — Tome

**Written:** 2026-07-28. Rewritten 2026-07-29 — **Stage 1 is complete and Stage 2 has started.**

> **CLAUDE.md still says "delete this file when Stage 1 lands", and Stage 1 has landed.** It was
> kept and rewritten rather than deleted because the "Traps already hit" section below is the only
> place several of these are written down. **Ask the owner** whether to fold that section into a
> permanent document (`docs/` somewhere) and delete the rest, or keep this note running through
> Stage 2. Do not delete it unilaterally.

---

## Where things stand, in one paragraph

Stage 0 and **all fifteen Stage 1 tickets are built and merged**. Three gate spikes have run for
real (SPIKE-002 reader bridge, SPIKE-010 legal posture, SPIKE-003 Tantivy scale). The whole
ingestion-to-reading path works end to end: `tome pull` fetches a site through the crawler (robots,
rate limit, SSRF), normalizes, sanitizes, localizes assets, relinks, and writes it to the library;
the app lists sources, renders a page from the stored AST, and shows it in a sandboxed iframe with
a library sidebar, a page outline, and back/forward. **Stage 2 (search) is now in progress: S2-2
landed 2026-07-29.** Search exists as a library — index, schema, tokenizer — but **nothing is wired
into the pipeline or the UI yet**, so the product still has no search a user can reach.

```
main   ← everything below is merged; branch from here (newest first)
  #40  S2-2 Tantivy integration + schema        ← Stage 2 starts here
  #41  checkout v7  ·  #7 setup-node v7  ·  #6 gitleaks v3  ·  #5 upload-artifact v7
  #39  SPIKE-003 Tantivy at 100k pages, measured
  #38  dependency majors, batched (and two refused)
  #32  S1-15 navigation + history
  #31  S1-14 three-panel layout, library sidebar, outline
  #30  S1-13 reader: renderer + page store + pipeline + iframe bridge
  #29  S1-12 typography, design tokens, contrast gate   ·   #28 S1-11 syntax highlighting
  #27  S1-10 asset localization + offline gate   ·   #26 S1-9 sanitizer
  #25  S1-8 normalization   ·  #24 S1-6 crawl  ·  #23 S1-5 SSRF  ·  #22 S1-7 parser
  #21  S1-4 HTTP client · #20 S1-2 DB · #19 S1-3 config · #16 S1-1 core types
  #17  SPIKE-010 legal posture   ·   #14 SPIKE-002 reader bridge
```

**The full pipeline**, all in `tome-core`, each stage a pure function over the previous:

```
config::SourceConfig::parse_file
  → crawl::Crawler::crawl            (robots + rate limit + SSRF + scope)
  → per page: normalize → sanitize → localize_assets → relink
  → store::PageStore (AST on disk) + db::Database (metadata)
  → render::render                   (AST → HTML, escaping contract)
  → reader iframe                    (one postMessage)
```

`pipeline::pull` composes it. **`tome pull <id>` and `tome list` work**; source configurations go
in `~/Library/Application Support/Tome/sources/<id>.yaml` by hand until P1-022.

---

## What to do next

**S2-1 — the relevance eval set and harness (P2-019).** It is the next ticket and it is the one the
rest of Stage 2 is measured by.

The plan's Stage 2 table lists S2-1 first, but S2-2 landed first, deliberately: P2-019 depends on
P2-001 because a relevance harness needs an index to score against. **The constraint that actually
matters is S2-1 before *ranking* — S2-4, S2-5, S2-6 — not S2-1 before integration.**
`docs/plans/18` now says this explicitly.

P2-019 wants ≥ 200 queries across ≥ 5 fixture sources, each labelled with acceptable target pages,
reporting MRR and recall@1/3/10 with per-query deltas, running offline against a fixed index, and
adding a query must be a one-line change to a YAML file.

**Field boosts in `search::schema::boost` are unmeasured placeholders and are labelled as such.**
Do not tune them by intuition — that is precisely what S2-1 exists to prevent. One test pins their
*direction* only.

---

## Stage 1's exit gate — met

The gate (`docs/plans/18` § Stage 1): *the app renders `docs.python.org` with the network off,
images included, anchors working, and the golden corpus committed.*

| | State |
|---|---|
| Renders `docs.python.org` | **Done.** 167 pages pulled for real (17 tutorial + 150 library) and read in the app |
| Images included | **Done.** 13 image references, all resolving to content-addressed local files through `tome://`; 0 remote, 0 dangling |
| Anchors working | **Done.** Outline links, scroll-spy, and `#fragment` navigation all exercised |
| Golden corpus committed | **Done.** 26 real pages, six platforms, licences verified per source |
| Network off | **Proven by test, not by pulling the plug.** `tests/reader_offline.rs` shuts the fixture server down and asserts the rendered HTML reaches for nothing. Nobody has literally turned the wifi off and read for an hour |

Two caveats that survive into Stage 2:

- **Frame pacing under bridge traffic is still unmeasured**, exactly as SPIKE-002 predicted — an
  occluded WKWebView suspends rAF entirely. Nothing in the reader gates on rAF for that reason
  (scroll reporting throttles on `performance.now()`), but the 60 Hz acceptance item needs eyes on
  a window.
- **A full-site `docs.python.org` crawl is not wanted** (owner, 2026-07-29). The scoped pulls
  demonstrated the gate; the local demo libraries under `/tmp` were removed. The Python pages in
  the golden corpus stay — those are test infrastructure, not a cached library.

**Try it now:**

```bash
mkdir -p /tmp/tome-demo/state/sources
cat > /tmp/tome-demo/state/sources/python-tutorial.yaml <<'YAML'
schema_version: 1
name: Python Tutorial
category: Python
source:
  type: generic
  url: https://docs.python.org/3/tutorial/
  generic:
    entry_points: ['index.html']
    max_pages: 40
    include_patterns: ['^/3/tutorial/']
fetch:
  rate_limit_rps: 2
YAML
TOME_HOME=/tmp/tome-demo ./target/debug/tome pull python-tutorial   # ~10s, 17 pages
open -n -a target/debug/bundle/macos/Tome.app --env TOME_HOME=/tmp/tome-demo
```

**Use `open -n -a`, not the binary directly.** Running
`target/debug/bundle/macos/Tome.app/Contents/MacOS/tome-app` from a shell gives a degenerate
91×100 window — the process is not registered as a GUI app. (And note `target/debug/Tome` is the
*CLI*: on a case-insensitive volume it is the same path as `target/debug/tome`.)

For a fully offline demo, serve the repo's own fixture instead — `(cd
crates/tome-testkit/fixtures/sphinx-example && python3 -m http.server 8731 &)`, point the config
at `http://127.0.0.1:8731/`, and set `fetch.allow_insecure: true`, which is what the SSRF filter's
owned-host exception is for.

## First thing to run

```bash
cd /Users/alexandernodeland/anodeland/projects/code/apps/tome
git checkout main && git pull
./scripts/check.sh --fast       # ~90s. Should print "All checks passed."
```

**`scripts/check.sh` is the gate — CI cannot run.** The repo is private until release *and* GitHub
Actions is blocked at the account level. The script runs exactly what `.github/workflows/ci.yml`
runs. **If you change one, change the other in the same commit** — drift here is the failure mode
this arrangement invites.

Note `npm audit` hits the live npmjs.org registry and **fails the gate when that endpoint is
down**, which it was for a stretch on 2026-07-29 (503s). A failing "deps: npm advisories" step with
a 503 in it is an outage, not a finding; re-run it.

## Read these, in order

1. `docs/plans/18-implementation-plan.md` — the execution plan. Every Stage 1 row is ✅; S2-2 too.
2. `docs/reviews/2026-07-28-plan-review.md` — what was wrong with the original plan and why.
3. `CLAUDE.md` — the settled facts that are easy to regress.
4. `docs/decisions/` — six ADRs. **Do not relitigate these.**
5. `docs/spikes/003-tantivy-scale.md` — Stage 2's whole design rests on its measurements.
6. `docs/spikes/002-reader-iframe-bridge.md` — the reader's, on its.

---

## Settled — do not reopen without a reason

| | Decision | Why it matters |
|---|---|---|
| ADR-0001 | Sync = iCloud Drive container with per-device op logs, **not CloudKit** | CloudKit is Swift-only; the core is Rust and the CLI runs out-of-process |
| ADR-0002 | **No App Sandbox.** Developer ID posture, hardened runtime | A sandboxed app and a Homebrew CLI would resolve *different* data dirs |
| ADR-0003 | Dual MIT OR Apache-2.0 | |
| ADR-0004 | Bundle id `com.alexnodeland.tome` | Threads through Keychain, iCloud, prefs, zap |
| ADR-0005 | Solo + Fable/Opus agent workflows; **sync deferred**; scope cut to S0–S4 | Changes the constraint from typing speed to verification bandwidth |
| ADR-0006 | **Unsigned**, via `alexnodeland/homebrew-tap`. Apple Developer Program deferred | Revisit at v1.0 |

Also settled, from the plan review — regressing any of these undoes real work:

- Tauri **is** the shell. No Swift shell. Reader = sandboxed `<iframe>`, not a second WKWebView.
- MCP is **stdio**. There is no `mcp.sock`.
- Local API: bearer token on **every** request including loopback; no CORS by default.
- Annotations anchor by quote+prefix/suffix, **never** bare character offsets.
- `robots.txt` obeyed by default, non-overridable for registry configs.
- **No telemetry, ever** — so no metric expressed as a percentage of users.
- `tome pull` fetches docs. There is **no `tome sync`**.
- Data lives in `~/Library/Application Support/Tome` + `~/Library/Caches/Tome`. Never `~/.tome`.

---

## The reader, as built — what a change here must not break

- **The renderer owes the sanitizer an escaping contract.** S1-9 deliberately does not
  charset-strip free-text fields (`Link.title`, `Image.alt`, `Admonition.title`, every text and
  code node); their safety is escaping, and `render.rs` is where it happens. **No `push_str` of
  page-derived text.** `crate::html` is the one escaping helper, and `attr` *quotes* as well as
  escapes.
- **`store.rs` names page files by a hash of the path, not by the path.** The default macOS volume
  is case-insensitive, so `Tutorial.html` and `tutorial.html` would be one file.
- **Internal links are library paths, not URLs.** `pipeline::relink` rewrites them, because a
  stored page that names a host only makes sense on the machine that crawled it, and ADR-0001
  syncs these trees between devices. A same-host page the crawl never reached stays absolute and
  is treated as external — that is the truth.
- **`page_base` is the exact inverse of `crawl::page_path_for`**, which is why that function is a
  shared free function. Getting this wrong once made every image on every page silently degrade to
  alt text.
- **The `tome://` handler is the one place a string from page content becomes a filesystem path.**
  It validates four things, including canonicalise-then-contain, which is what catches a symlink.
- **The frame is `sandbox="allow-scripts"` and nothing else.** `allow-same-origin` would hand page
  content the app's origin and with it `__TAURI_INTERNALS__`.
- **The frame's CSP names the app origin explicitly.** `'self'` in an opaque origin matches nothing.
- **Nothing gates on `requestAnimationFrame`.** An occluded WKWebView suspends it entirely.
- **The scroll-spy runs in the frame**, because the app cannot read that document.
- **Highlighting is a render concern.** `CodeBlock` stays `{language, code}`; classes, not colours,
  so a theme swap needs no re-highlighting.

## Search, as built (S2-2) — what a change here must not break

- **`IndexSession::commit` reloads the reader synchronously, and must keep doing so.**
  `ReloadPolicy::OnCommitWithDelay` reloads on a background thread *after a delay*, so a search
  issued right after our own commit returns the pre-commit view. Every "the page I just indexed
  cannot be found" symptom starts here, and it is timing-dependent — a test can pass while the
  product is broken. The policy is still there for the cross-process case (CLI indexes while the
  app is open); the explicit reload covers our own writes.
- **A session must be dropped, not merely shadowed.** Tantivy holds a directory lock for the
  writer's lifetime, and `let mut session = engine.session()?` over an existing binding keeps the
  first alive to end of scope, so the second fails with `LockBusy`.
- **Only `source_id`/`path`/`title` are STORED**, and that is what keeps the index at SPIKE-003's
  measured 224 MB per 100k pages. Storing `body` would roughly double it to hold a second copy of
  what `PageStore` already has. The consequence: snippets (P2-005) **cannot** use Tantivy's
  `SnippetGenerator`, which needs a stored field — they must re-read from the store, which is
  better anyway because the store has structured nodes.
- **The code tokenizer is registered on the `Index`, not on a writer.** `QueryParser` resolves
  tokenizers through the same manager; registering only on the write path analyses queries with
  the default tokenizer and returns nothing for every code search, with no error attached.
- **`extract.rs` has no catch-all match arm, on purpose.** `Node` is `#[non_exhaustive]`, but that
  binds only *other* crates — inside `tome-core` the match must stay exhaustive so a new variant
  fails the build and forces a decision about which field its text belongs in.
- **Boosts are unmeasured placeholders.** See "What to do next".

---

## Traps already hit — don't rediscover these

Each of these cost real time. They are fixed; this is so a future change doesn't reintroduce them.

**Content fidelity — nine defects, every one found by looking at a real page**

Not one was caught by a test, because a test written from the same misunderstanding as the code
agrees with it. The golden corpus (26 real pages, six platforms) is the standing defence.

- **A copy button inside `<pre>`.** Node's API docs put their copy-button strip *inside* the
  code block, so every example ended in `jscopy` / `jsoncopy` — 37 of them. The `pre` arm now
  reads the `<code>` element, falling back to a chrome-skipping walk for Sphinx, which has no
  `<code>` and whose spans *are* the code.
- **Permalinks are chrome anywhere**, not just in headings: rustdoc's per-method `§`, mdBook's
  footnote `↩`. And dropping one must **leave its `id` behind** — Node's
  `<a id="osarch" href="#osarch">#</a>` is the deep-link target for that API entry.
- **A permalink marker is not always `¶`.** Node uses `#`, so every Node page was titled `OS#`.
  `PERMALINK_MARKERS` is a list for that reason.
- **Breadcrumbs and end-of-page furniture** survive into the content root (`std::fs`,
  "Was this page helpful?"). `hidden` and `aria-hidden` are the HTML's own signals and need no
  class list; the rest is a documented substring list, kept safe by the corpus.
- **`split_whitespace().join(" ")` deletes boundary whitespace.** It was collapsing runs *and*
  trimming, so every space next to an inline element vanished: `the interactive <a>REPL</a>` →
  "the interactiveREPL", on every page of every source. Inline prose now uses
  `collapse_inline_ws`; blocks trim their own edges via `tidy_block_children`.
- **The mirror image:** merging adjacent text fragments with an inserted space turned `a&amp;b`
  into "a & b". html5ever splits text around entities; each fragment carries its own whitespace.
- **mdBook wraps whole headings in a self-link**, so they render as giant underlined links unless
  `unwrap_self_permalink` unwraps them. Sphinx puts the permalink beside the text; mdBook wraps
  the text in it.
- **Alphabetical page order is not document order.** The crawler already visits pages in
  navigation order; `pages.ordinal` records it. Sorting by path opened the Cargo Book on its
  changelog.
- **An offline assertion passes trivially when there is nothing left to leak.** The
  no-remote-`src` check was green while *every* image was silently degrading to alt text. The
  test now counts rendered images.
- **The offline gate is about subresources, not links.** An `<a href>` is inert until clicked.
- **`grid-template-rows: auto auto 1fr` assigns tracks by child order.** With a conditional
  banner between the header and the panels, the panels landed in an `auto` track and the shell
  only filled the window when its content happened to be tall enough. Flex column instead.

**The reader (from S1-11..S1-15)**

- **A synthesised base URL breaks asset localization silently.** Normalization absolutises URLs
  against whatever base it is given; give it a fake one and every relative asset becomes an
  unfetchable scheme, the sanitizer rejects it, and images degrade to alt text with no error.
- **`"the string is absent"` is the wrong shape of assertion for escaping.**
  `!html.contains("onload=")` fails on *correct* output, because the payload's text appears safely
  escaped inside a quoted value. The right assertion is "no tag came from the input".
- **Scanning for `=` to check attribute quoting is wrong** the moment a value contains an `=`,
  which documentation routinely does. The properties use a real tag scanner.
- **jsdom under this Vitest config exposes no `localStorage`.** `window.localStorage` is
  `undefined`. `src/test/setup.ts` provides an in-memory `Storage`; without it, persistence tests
  silently assert the fallback path.
- **`userEvent.keyboard` reserves `[` and `]`** for key codes. Dispatch a `KeyboardEvent` directly.
- **A test of `open_external` launches a real browser.** Validation is split into
  `validate_external` so the allowlist is testable without side effects.
- **`target/debug/Tome` is the CLI, not the app** — case-insensitive filesystem, `tome` and `Tome`
  are one path. The app binary is `target/debug/tome-app`, bundled at
  `target/debug/bundle/macos/Tome.app/Contents/MacOS/tome-app`.
- **syntect's bundled syntax set has no TypeScript** (and no TOML, Dockerfile, Kotlin, Swift).
  `typescript`/`tsx`/`jsx` alias to JavaScript; the rest render plain. `two-face` is the upgrade
  path and is a *licence* decision, not a technical one.
- **`bincode` 1.3.3 arrives with syntect** and carries an unmaintained advisory
  (RUSTSEC-2025-0141), listed individually in `deny.toml` with dated reasons.

**Search (from S2-2)**

- **`TopDocs` is a builder in tantivy 0.26.** Only `.order_by_score()` implements `Collector`, and
  `with_limit` **panics on 0**, so a caller-supplied limit must be clamped.
- **`MmapDirectory::open` returns its own error type**, not `TantivyError`.
- **Tantivy enters at default features, unlike syntect.** SPIKE-003 measured it that way and the
  adoption case is those numbers. `stemmer` in particular changes which documents match, so it is
  a *relevance* decision belonging to S2-4 scored against S2-1 — not a feature-flag judgement.
  `columnar-zstd-compression` pulls `zstd-sys` (C); that is not a new constraint, because
  `rusqlite`'s `bundled` already compiles SQLite from C. **The syntect comment's "no C toolchain
  assumptions" claim was already inaccurate when written.**

**Test infrastructure (from S0-6/7/8)**

- **macOS accepted sockets inherit `O_NONBLOCK` from the listener** (BSD behaviour; Linux does
  not). Every accepted stream needs an explicit `set_nonblocking(false)`.
- **Percent-decode before checking for `..`**, not after. Both the fixture server and the
  `tome://` handler do this.
- **Fixtures and corpora must be excluded from Prettier *and* ESLint.**
- **`fuzz/` needs `[workspace]` in its own `Cargo.toml` *and* `exclude = ["fuzz"]` in the root.**
- A proptest generator for "arbitrary absolute root" will happily generate `/~`.

**App shell (from SPIKE-002 — details in `docs/spikes/002-reader-iframe-bridge.md`)**

- **Tauri core APIs are deny-by-default and fail silently when the rejection is not awaited.**
  `src-tauri/capabilities/default.json` grants `core:event:default` only; extend it deliberately.
- **An occluded window suspends rAF entirely and clamps timers to ~230 ms** (WKWebView).
- **`'self'` in a CSP inside the sandboxed frame matches nothing** — the origin is opaque.

**Build / tooling**

- **`Icon?` in the user's global gitignore matches `icons/`** case-insensitively on macOS.
  `.gitignore` has explicit `!src-tauri/icons/` negations. **Clone fresh before any release.**
- **Svelte 5 + Vitest** resolves the *server* build; `conditions: ['browser']` scoped to
  `process.env.VITEST` fixes it.
- **`$lib` needs two aliases**: `paths` in `tsconfig.json` *and* `resolve.alias` in `vite.config.ts`.
- `vite.config.ts` must import `defineConfig` from **`vitest/config`**.
- Workspace lints **deny `unwrap`/`expect`/`panic`**. Test files need a file-level
  `#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]`.
- **`no-undef` is off for TS and Svelte** — it cannot see DOM lib types and TypeScript already
  resolves every identifier.
- **Prettier deliberately excludes markdown.** Don't "fix" this.
- **`gh` needs the `workflow` scope** to merge a PR touching `.github/workflows/`. Without it the
  merge fails with a GraphQL error naming the scope. `gh auth refresh -h github.com -s workflow`.

**macOS / distribution**

- **`Apple Development` ≠ `Developer ID Application`.** The cert on this machine cannot sign for
  distribution.
- **macOS 15 removed the Control-click→Open Gatekeeper bypass.** Caveats must lead with
  `xattr -dr com.apple.quarantine`.
- `brew style` **refuses to lint a cask outside a tap.**
- The built app is `adhoc, linker-signed`. `spctl` rejects it. Expected.

**cargo-deny**

- Graph is scoped to `aarch64-apple-darwin`, which excludes Tauri's Linux GTK backend.
- Unmaintained advisories are listed **individually with reasons**, deliberately not
  `unmaintained = "warn"`, so a *new* one still fails the build. It has caught one already.
- **Tantivy's tree added no new ignores** — it passed clean on first run.

---

## Open — needs the user, don't decide alone

- **This file's fate.** See the note at the top.
- **`two-face`** for TypeScript/TOML syntax highlighting — a licence decision.
- **DEC-005** docset import priority · **DEC-006** `watch` fetch vs notify · **DEC-007** note
  format · **DEC-008** export targets. All non-blocking.
- **PR #2** — the stale scaffolding branch from January, superseded by everything since. Still
  open; ask before closing.
- **PR #10 (TypeScript 7)** — deliberately not merged, twice now. `npm ci` fails outright:
  `svelte-check@4.7.4` peers on `typescript@^5 || ^6`. It is a reminder, not a task. Re-check when
  svelte-check and typescript-eslint support TS 7.
- **Going public + Actions billing.** Until then CI stays red-but-meaningless: every run fails in
  ~2 s without executing a step, so **PR checks carry no information**. Judge by `check.sh`.
- **Suggested to them, no response yet:** backport the macOS 15 Gatekeeper caveats fix to
  `curio.rb` in their tap.

## Standing decisions (owner, 2026-07-28)

- **Refute-panel is the standing method for security-critical tickets** — run the Workflow
  automatically (no re-ask); majority-refute gates the merge. Durable opt-in.
- **Keep merging green PRs ticket-by-ticket**, as long as the gate is green.
- **Real golden-corpus expansion was deferred until the reader renders.** It renders; the corpus
  is 26 real pages across six platforms as of 2026-07-29.

## Working with this user

- They want **costs stated, not glossed**. Keep that register.
- Route by task shape: Fable for exactly-specified and loudly-verified work, **Opus for anything
  that fails silently** (sanitize, ssrf, paths, render, sync, auth) and for every verification pass.
- They have prior art worth checking before inventing: `alexnodeland/homebrew-tap`.
- **Don't screenshot the app** unless the window can be targeted reliably — a `screencapture`
  window-id lookup once silently fell back to full-screen and caught an unrelated session.
  Functional verification is better evidence anyway.

## Verified environment

Rust 1.96.1 · Node 26.3.0 · npm 11.16.0 · tauri-cli 2.5.0 · macOS 26.5 · arm64 · Xcode CLT present.
`cargo-deny` installed; `cargo-audit` **not** installed (the script skips it and says so).
`cargo-fuzz` and a nightly toolchain are **not** installed — which is why the `highlight` and
`render` fuzz targets are mirrored as proptest properties that do run in the gate.

369 Rust tests (workspace, counted 2026-07-29) + 66 Vitest tests + 9 fuzz targets; the
normalization corpus is 26 real pages.
`npm run tauri build --debug` produces `Tome.app`.
