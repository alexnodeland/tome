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
- **`Ranking::TUNED` is measured, and changing it by intuition throws the measurement away.** S2-4
  set every value in it by coordinate descent against S2-1's eval set. Two of them look wrong and
  are not: `title` (0.75) sits *below* `body`, because a title is a handful of tokens and BM25's own
  length normalisation already multiplies it heavily — an explicit boost on top was double-counting;
  and `code` (1.0) sits below `headers`, because on these platforms method names are also headings.
  Re-run `sweep_ranking_parameters` before touching any of it, and update `baseline.json` in the
  same change.
- **The whole product searches with `Ranking::TUNED`.** `search_with` exists for the sweep alone.
  A second configuration reaching users would make the baseline describe a ranking nobody gets.

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
- **A relevance eval is only as sensitive as its corpus is large, and 26 documents is not large.**
  At that size, removing an entire indexed field from the query moved MRR by 0.0036 and tripped
  nothing — the metrics compress near the top because there is rarely a strong wrong answer for a
  right one to beat. At 339 documents the same class of change moves MRR by 0.32. **Never read a
  relevance number without knowing the corpus size**, and do not "improve" a score by shrinking or
  easing the corpus. The Stage 2 exit gate carries a ≥ 150 document floor for this reason and the
  harness asserts it.
- **Expanding an eval corpus makes existing labels incomplete, not wrong.** Going 26 → 339 dropped
  MRR from 0.92 to 0.72, and a large part of that was queries whose *correct* answer was now a page
  that had not existed in the corpus before (`node:api/url.html` for "URL", `node:api/modules.html`
  for "modules"). Relabel by reading actual results — `TOME_RELEVANCE_DUMP=1` prints them — but
  only add a target that genuinely answers the query. Labelling a bad-but-high-ranking result to
  make the number go up destroys the instrument.
- **"What is indexed?" must be asked of the index, never of the database.** They live under
  different roots — the database in state, the index in cache — so clearing the cache, or macOS
  evicting it under disk pressure, leaves a full database and an empty index. A sync that trusted
  the database would answer "everything is indexed, nothing to do" and leave search permanently
  empty **with no error anywhere**. `SearchEngine::indexed_pages` reads the index, which makes the
  sync self-correcting.
- **Tantivy has no update.** Re-adding a changed page without deleting the old document leaves
  *both*, and the page appears twice in every result list. `IndexSession::delete_page` must be
  called before re-adding.
- **A page's identity is `(source, path)`, not `path`.** `index.html` exists in nearly every
  source; deleting by path alone would empty the others.
- **`tome pull` does not prune pages that vanished upstream**, deliberately. A crawl stops early
  for reasons that have nothing to do with the site — page cap, dropped network, closed laptop —
  and treating "not seen this run" as "deleted upstream" would delete a user's library a few
  hundred pages at a time. Content is hours of polite crawling; the index that reads it is seconds
  to rebuild. The policy was agreed on 2026-07-29 — prune **only** when the crawl completed with no
  errors and without hitting the page cap — and is recorded on `Database::delete_page`, but is
  **not implemented**. Whoever implements it owes the test that a capped or errored crawl deletes
  nothing; that guard is the whole point and it fails silently.
- **`TopDocs` is a builder in tantivy 0.26.** Only `.order_by_score()` and `.tweak_score(…)`
  implement `Collector`, and `with_limit` **panics on 0**, so a caller-supplied limit must be
  clamped.
- **BM25's `k1` and `b` are private constants in tantivy 0.26**, not weight parameters. The
  textbook lever for "one enormous page outranks every specific one" is raising `b`, and it is
  unreachable without forking the crate. `Ranking::length_penalty` applies a post-hoc divisor from
  the collector instead. The plan described the `b` lever before anyone checked; do not go looking
  for `set_bm25_params`.
- **Coordinate descent cannot move a pair of multiplicative parameters.** `length_pivot` and
  `length_penalty` are inert unless *both* are non-zero, so a sweep that varied each alone from a
  zero start found both worthless and reported, confidently, that length normalisation did not
  help. Sweeping them as one joint coordinate moved MRR by 0.019 — the second-largest single gain
  in S2-4. **Any parameter that only acts in combination with another must be swept jointly.**
- **A synthetic ranking test is very easy to write so that it does not test the thing it is named
  for.** Two of them here did. `title_outranks_body` omitted the h1 that normalization guarantees
  every stored page has, so it was really testing the `title` field alone and duly failed when
  tuning moved weight to `headers` — on a page shape that cannot occur. The book-dump test's first
  draft did not reproduce the defect at all, because a real concatenated book carries every
  chapter's *headings* too. **Assert the control case**: that with the mechanism disabled, the bad
  outcome happens. That is what caught both.
- **`FuzzyTermQuery` is a constant-score query.** It is built on an `AutomatonWeight`, which
  produces a `ConstScorer`, so *every* document it matches scores identically — BM25 is discarded
  for that term, and none of `Ranking`'s tuning reaches it. P2-009's technical note sketches
  exactly this and it does not survive contact with how tantivy scores. `search::fuzzy` corrects
  the **query** against the term dictionary instead, so a correction is searched as an ordinary
  term. "Did you mean?" then falls out for free, which a query relaxation cannot give you: a
  `FuzzyTermQuery` never reveals which terms it matched.
- **Typo correction is prefix-anchored and therefore blind to typos in the first three
  characters.** `teh` will not find `the`. This is a deliberate cost — without a prefix, correcting
  one term means reading the whole term dictionary — and it is pinned by a test so it fails there
  rather than surprising someone. Removing the blind spot means taking `levenshtein-automata` and
  `tantivy-fst` as direct dependencies pinned to whatever tantivy resolves, because `DfaWrapper` is
  `pub(crate)`.
- **Corrections are drawn from prose fields only, never `code`.** Identifier vocabularies have
  dense near neighbours — `read_dir`/`read_din`, `parseInt`/`parseInto` — so a correction towards
  one is likelier wrong than helpful.
- **P2-009's distance schedule cannot reach every real typo, and that is the specification's call,
  not a bug.** `modual` → `module` is 2 edits on a 6-character term (1 allowed) and `pth` → `path`
  is 1 edit on a 3-character term (0 allowed). Both are in the eval corpus and both are expected to
  fail. Widening the schedule buys false positives everywhere: at distance 1 on three-character
  terms, `Vec` reaches `Vex`, `Vev`, `sec` and `hex`.
- **Do not close a relevance gap by choosing the parameter that crosses it.** During S2-5 recall@3
  sat at 0.8986 against the 0.90 gate — one query of 207 — and a neighbour reached 0.9034 while
  costing symbol accuracy. It was rejected: fitting a gate is not passing it. S2-6 then cleared the
  gate at 0.9082 with a configuration that was the optimum on a *different* objective and dominated
  its predecessor on every column, which is what passing looks like. The same goes for re-labelling
  queries to make the number rise: that destroys the instrument, which is the one thing here that
  cannot be rebuilt in seconds. Differences of one or two queries are inside the noise.
- **Documentation sites do not declare their API in code blocks.** P2-015's technical note regexed
  `fn\s+(\w+)` out of code blocks; measured over the 339-page corpus that yields `main`, `buf`,
  `server`, `Foo`, `__init__`, `options` — the *examples'* scaffolding. `Vec` is declared once and
  mentioned 321 times; `with_capacity` is declared **never**. The signatures are in **headings**
  (`h4: pub fn with_capacity(capacity: usize) -> Vec<T>`), the kind is in the rustdoc **path**
  (`struct.Vec.html`), and `search::symbols` reads those. This is the same fact behind S2-4's
  finding that the `code` field barely matters.
- **A field of every declaration is noise; a field of the page's one primary symbol is signal.**
  Blending all declarations at boost 3.0 cost 0.08 MRR and 39 queries, because every rustdoc page
  declares `from`, `into`, `borrow`, `fmt` and `try_from` as trait boilerplate and a *short* field
  makes each a strong BM25 signal. Coordinate descent drove that boost to zero. The split is
  `symbol` (primary, blended) versus `declarations` (all, reachable only by `@symbol`) — and
  `declarations` must not be added to the ordinary query's field list. A test pins that.
- **Changing `search::schema::build` makes every existing index unreadable.** Tantivy's
  `open_or_create` returns `SchemaError`, which maps to `Error::IndexSchemaOutdated` so a *read*
  command can name the remedy instead of deleting an index the user did not ask it to touch. Only
  `open_or_rebuild` may discard. Adding a field is therefore a migration, cheap only because the
  index lives in the cache.
- **Stopwords are dropped from the query, never from the index**, so IDF is untouched and the
  policy can be changed or reverted with no reindex. Two refusals in `StopwordPolicy::apply` are
  load-bearing: a query containing a quote is returned untouched (a phrase with a hole matches
  nothing), and a query that is *entirely* stopwords is returned untouched (an empty query looks
  like a broken index). `in`, `is`, `not`, `and`, `or`, `for`, `if`, `return` and `type` are
  deliberately **not** stopwords — they are keywords in the documented languages, and a docs reader
  that cannot look up Python's `in` operator has traded a real capability for a marginal one.
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
