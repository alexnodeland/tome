# Traps — mistakes already made in this repository

**Every entry here cost real time once.** They are all fixed. This file exists so a later change
does not reintroduce them, and so nobody spends an afternoon rediscovering something that is
already known.

This is **not** status, and it is not a plan. It carries no branch state, no ticket list, and no
"what to do next" — those go stale in days and belong in `docs/plans/` and in git. Everything here
should still be true a year from now. If an entry stops being true, delete it rather than
qualifying it.

Two kinds of entry:

- **Invariants** — properties the code currently has that a plausible change would silently break.
  These describe working code, and the note is about what would go wrong if you changed it.
- **Traps** — defects that actually happened, with the shape of the mistake preserved.

The common thread is that almost none of these fail loudly. That is why they are written down.

---

## Invariants — the reader

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
  is treated as external — that is the truth, not a bug.
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

## Invariants — search

- **`IndexSession::commit` reloads the reader synchronously, and must keep doing so.**
  `ReloadPolicy::OnCommitWithDelay` reloads on a background thread *after a delay*, so a search
  issued right after our own commit returns the pre-commit view. Every "the page I just indexed
  cannot be found" symptom starts here, and it is timing-dependent — a test can pass while the
  product is broken. The policy is still there for the cross-process case (the CLI indexing while
  the app is open); the explicit reload covers our own writes.
- **A session must be dropped, not merely shadowed.** Tantivy holds a directory lock for the
  writer's lifetime, so `let mut session = engine.session()?` over an existing binding keeps the
  first alive to the end of the scope and the second fails with `LockBusy`.
- **Only `source_id`/`path`/`title` are STORED**, and that is what keeps the index at SPIKE-003's
  measured 224 MB per 100k pages. Storing `body` would roughly double it to hold a second copy of
  what `PageStore` already has. The consequence: snippets (P2-005) **cannot** use Tantivy's
  `SnippetGenerator`, which requires a stored field — they must re-read from the store, which is
  the better place anyway, because the store holds structured nodes and a snippet can respect
  block boundaries.
- **The code tokenizer is registered on the `Index`, not on a writer.** `QueryParser` resolves
  tokenizers through the same manager; registering only on the write path analyses queries with
  the default tokenizer and returns nothing for every code search, with no error attached.
- **`search::extract` has no catch-all match arm, on purpose.** `Node` is `#[non_exhaustive]`, but
  that binds only *other* crates — inside `tome-core` the match must stay exhaustive so a new
  variant fails the build and forces a decision about which field its text belongs in. A `_ => {}`
  would route new content to nowhere, discoverable only as a page that cannot be found by words it
  visibly contains.
- **The field boosts in `search::schema::boost` are unmeasured placeholders**, labelled as such.
  Tuning them belongs to S2-4, scored against S2-1's eval set. Changing them by intuition is
  exactly what building the eval set first is meant to prevent.

---

## Traps — content fidelity

Nine defects, every one found by looking at a real page. **Not one was caught by a test**, because
a test written from the same misunderstanding as the code agrees with it. The golden corpus
(26 real pages, six platforms) is the standing defence.

- **A copy button inside `<pre>`.** Node's API docs put their copy-button strip *inside* the code
  block, so every example ended in `jscopy` / `jsoncopy` — 37 of them. The `pre` arm now reads the
  `<code>` element, falling back to a chrome-skipping walk for Sphinx, which has no `<code>` and
  whose spans *are* the code.
- **Permalinks are chrome anywhere**, not just in headings: rustdoc's per-method `§`, mdBook's
  footnote `↩`. And dropping one must **leave its `id` behind** — Node's
  `<a id="osarch" href="#osarch">#</a>` is the deep-link target for that API entry.
- **A permalink marker is not always `¶`.** Node uses `#`, so every Node page was titled `OS#`.
  `PERMALINK_MARKERS` is a list for that reason.
- **Breadcrumbs and end-of-page furniture** survive into the content root (`std::fs`, "Was this
  page helpful?"). `hidden` and `aria-hidden` are the HTML's own signals and need no class list;
  the rest is a documented substring list, kept safe by the corpus.
- **`split_whitespace().join(" ")` deletes boundary whitespace.** It was collapsing runs *and*
  trimming, so every space next to an inline element vanished: `the interactive <a>REPL</a>` →
  "the interactiveREPL", on every page of every source. Inline prose now uses `collapse_inline_ws`;
  blocks trim their own edges via `tidy_block_children`.
- **The mirror image:** merging adjacent text fragments with an inserted space turned `a&amp;b`
  into "a & b". html5ever splits text around entities; each fragment carries its own whitespace.
- **mdBook wraps whole headings in a self-link**, so they render as giant underlined links unless
  `unwrap_self_permalink` unwraps them. Sphinx puts the permalink *beside* the text; mdBook wraps
  the text *in* it.
- **Alphabetical page order is not document order.** The crawler already visits pages in
  navigation order and `pages.ordinal` records it. Sorting by path opened the Cargo Book on its
  changelog.
- **An offline assertion passes trivially when there is nothing left to leak.** The no-remote-`src`
  check was green while *every* image was silently degrading to alt text. The test now counts
  rendered images.
- **The offline gate is about subresources, not links.** An `<a href>` to example.com is inert
  until clicked; an `<img src>` fetches on render. Asserting "no https anywhere" fails on a page
  that merely links out, which is correct behaviour.
- **`grid-template-rows: auto auto 1fr` assigns tracks by child order.** With a conditional banner
  between the header and the panels, the panels landed in an `auto` track and the shell only filled
  the window when its content happened to be tall enough. Flex column instead.

## Traps — the reader

- **A synthesised base URL breaks asset localization silently.** Normalization absolutises URLs
  against whatever base it is given; give it a fake one and every relative asset becomes an
  unfetchable scheme, the sanitizer rejects it, and images degrade to alt text with no error.
  Caught only because the offline test counts rendered images.
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
- **`target/debug/Tome` is the CLI, not the app** — on a case-insensitive filesystem `tome` and
  `Tome` are one path. The app binary is `target/debug/tome-app`, bundled at
  `target/debug/bundle/macos/Tome.app/Contents/MacOS/tome-app`.
- **Launch the app with `open -n -a`, not the binary directly.** Running the bundled executable
  from a shell gives a degenerate 91×100 window, because the process is not registered as a GUI app.
- **syntect's bundled syntax set has no TypeScript** (and no TOML, Dockerfile, Kotlin, Swift).
  `typescript`/`tsx`/`jsx` alias to JavaScript; the rest render plain. `two-face` is the upgrade
  path and is a *licence* decision, not a technical one.

## Traps — search

- **User query text is not query syntax, and Tantivy's parser disagrees.**
  `(`, `)`, `[`, `]`, `:`, `+`, `-`, `^`, `~`, `*`, `?`, `/`, `\`, `{`, `}` are all operators, as
  are the bare words `AND`, `OR`, `NOT`, `IN`. Passing a search box's contents straight to
  `QueryParser` means `os.cpus()` matches nothing (a term plus an empty group), `[features]` is a
  malformed range, `Vec::new` resolves *field* `Vec` and errors, and `C++` is a parse error.
  Typing a function's call syntax is the most natural way to search API documentation, and every
  such query silently failed. `search::plain_text_query` neutralises this; **anything that builds a
  query must go through it.** Found by the S2-1 eval set on its first run — twelve dead symbol
  queries, and symbol recall@1 went 0.7465 → 0.9474 once fixed.
- **The relevance gate is a strong diagnostic and a weak gate, and this is measured, not assumed.**
  Cutting the title boost 60×, additionally cutting the code boost 1500×, and removing the `code`
  field from the query entirely each moved MRR by ≤ 0.0036 and tripped nothing. A green relevance
  run means "nothing catastrophic", not "ranking is fine". The per-query movement report is the
  part that works. Making it a real gate needs a bigger corpus, not a tighter threshold.
- **`TopDocs` is a builder in tantivy 0.26.** Only `.order_by_score()` implements `Collector`, and
  `with_limit` **panics on 0**, so a caller-supplied limit must be clamped.
- **`MmapDirectory::open` returns its own error type**, not `TantivyError`.
- **Tantivy enters at default features, unlike syntect.** SPIKE-003 measured it that way and the
  adoption case is those numbers. `stemmer` in particular changes which documents match, so it is a
  *relevance* decision belonging to S2-4 scored against S2-1 — not a feature-flag judgement call.
  `columnar-zstd-compression` pulls `zstd-sys` (C); that is not a new constraint, because
  `rusqlite`'s `bundled` already compiles SQLite from C. **The syntect comment's "no C toolchain
  assumptions" claim was already inaccurate when it was written.**

## Traps — test infrastructure

- **macOS accepted sockets inherit `O_NONBLOCK` from the listener** (BSD behaviour; Linux does
  not). Every accepted stream needs an explicit `set_nonblocking(false)`.
- **Percent-decode before checking for `..`**, not after. Both the fixture server and the `tome://`
  handler do this.
- **Fixtures and corpora must be excluded from Prettier *and* ESLint.**
- **`fuzz/` needs `[workspace]` in its own `Cargo.toml` *and* `exclude = ["fuzz"]` in the root.**
- A proptest generator for "arbitrary absolute root" will happily generate `/~`.

## Traps — app shell

Details in [`docs/spikes/002-reader-iframe-bridge.md`](../docs/spikes/002-reader-iframe-bridge.md).

- **Tauri core APIs are deny-by-default and fail silently when the rejection is not awaited.**
  `src-tauri/capabilities/default.json` grants `core:event:default` only; extend it deliberately.
- **An occluded window suspends rAF entirely and clamps timers to ~230 ms** (WKWebView).
- **`'self'` in a CSP inside the sandboxed frame matches nothing** — the origin is opaque.

## Traps — build and tooling

- **`Icon?` in a global gitignore matches `icons/`** case-insensitively on macOS. `.gitignore` has
  explicit `!src-tauri/icons/` negations. **Clone fresh before any release.**
- **Svelte 5 + Vitest** resolves the *server* build; `conditions: ['browser']` scoped to
  `process.env.VITEST` fixes it.
- **`$lib` needs two aliases**: `paths` in `tsconfig.json` *and* `resolve.alias` in
  `vite.config.ts`.
- `vite.config.ts` must import `defineConfig` from **`vitest/config`**.
- Workspace lints **deny `unwrap`/`expect`/`panic`**. Test files need a file-level
  `#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]`.
- **`no-undef` is off for TS and Svelte** — it cannot see DOM lib types, and TypeScript already
  resolves every identifier.
- **Prettier deliberately excludes markdown.** Don't "fix" this.
- **`npm audit` hits the live npmjs.org registry**, so the gate's "deps: npm advisories" step fails
  when that endpoint is down (503s, observed 2026-07-29). A 503 is an outage, not a finding —
  re-run it.
- **`gh` needs the `workflow` scope** to merge a PR touching `.github/workflows/`; without it the
  merge fails with a GraphQL error naming the scope. `gh auth refresh -h github.com -s workflow`.
- **CI carries no information while Actions is blocked at the account level.** Every run fails in
  ~2 seconds without executing a step, so a red PR check means nothing. Judge by
  `./scripts/check.sh`.
- **TypeScript 7 is not adoptable** while `svelte-check` peers on `typescript@^5 || ^6` — `npm ci`
  fails outright, not merely with warnings. Re-check when svelte-check and typescript-eslint
  support it.

## Traps — macOS and distribution

- **`Apple Development` ≠ `Developer ID Application`.** The certificate on the maintainer's machine
  cannot sign for distribution.
- **macOS 15 removed the Control-click→Open Gatekeeper bypass.** Cask caveats must lead with
  `xattr -dr com.apple.quarantine`.
- `brew style` **refuses to lint a cask outside a tap.**
- The built app is `adhoc, linker-signed`. `spctl` rejects it. Expected.

## Traps — cargo-deny

- The graph is scoped to `aarch64-apple-darwin`, which excludes Tauri's Linux GTK backend.
- Unmaintained advisories are listed **individually, with dated reasons**, deliberately not
  `unmaintained = "warn"`, so a *new* one still fails the build. It has caught one already
  (`bincode` 1.3.3, arriving with syntect, RUSTSEC-2025-0141).
- **Tantivy's tree added no new ignores** — it passed clean on first run.
