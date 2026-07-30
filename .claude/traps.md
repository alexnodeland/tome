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

## Traps — the search UI

- **A snippet is crawled page content, and it is drawn in the *app's* DOM.** The reader's HTML is
  safe because it lives in a sandboxed iframe with an opaque origin; a snippet does not, so the
  app's origin and its IPC bridge are reachable from it. That is why `search::snippet` returns
  `Vec<Span>` — text plus a boolean — instead of a marked-up string, and why there is **no
  `{@html}` in `SearchModal.svelte` and must never be one**. A test pins it by feeding a snippet
  containing `<script>` and asserting no element is created.
- **A debounce does not serialise requests.** A slow query issued at keystroke 3 can resolve after
  a fast one issued at keystroke 5, and the results list would then answer a question the user has
  finished changing. Every search carries a sequence number and a stale response is dropped.
- **A remembered scope must be revalidated on load.** A source can be removed between launches, and
  a scope naming one that is gone returns nothing for ever with no error anywhere — the same class
  of silent-empty failure as trusting the database for what is indexed.
- **Highlight the terms that were *searched*, not the ones that were typed.** After a typo
  correction they differ, and marking `enviroment` marks nothing, so a correct result looks
  unrelated to the query. `SearchEngine::highlight_terms` returns the searched set: stopwords
  dropped, `@` sigils stripped, corrections added.
- **`scrollIntoView` is not available in every DOM the app renders into.** jsdom has no layout and
  so no scrolling. An unguarded call from a *cosmetic* scroll produced a real unhandled rejection.
- **A `#[tauri::command]` cannot be unit-tested**, because `tauri::State` cannot be constructed.
  Commands here are thin wrappers over plain functions taking `&Paths`; a command that does its
  work inline is a command that is never tested.

- **`event.key` is uppercase when Shift is held.** `event.key === 'g'` never fires for ⇧⌘G, so
  every Shift-bearing shortcut in Appendix C — and there are three — silently did nothing.
  `isCommand` now excludes Shift and `isCommandShift` matches it, both comparing
  case-insensitively. The two are mutually exclusive so one event cannot fire two actions.
- **Nothing in the app can read the reader's document.** The frame is `sandbox="allow-scripts"`
  with no `allow-same-origin`, so its origin is opaque: `window.find()`, `querySelector` and
  `getSelection` from the shell all operate on the app's chrome. Anything that needs the page's
  text — in-page find, and future annotation anchoring — runs in `public/reader-frame.js` and
  answers by `postMessage`.
- **Do not wrap matches in `<mark>`.** `Range.surroundContents` throws whenever a range partially
  covers a node, which is the *normal* case for a match crossing the `<span>`s the syntax
  highlighter emits. The CSS Custom Highlight API paints ranges without touching the DOM, which
  also keeps the standing rule that highlighting is a render concern and not a mutation. It is
  feature-detected and the result is reported, so a missing API shows "unavailable" rather than
  "no matches" — a search that never ran must not claim the page is empty.
- **A cosmetic failure must not take the answer with it.** `Range.getBoundingClientRect` does not
  exist in every DOM, and an unguarded call from the scroll-to-match threw *out of the message
  handler*, before the reply was sent — so the find bar reported "no matches" for a page full of
  them. Same class as the `scrollIntoView` guard in the search modal.
- **The frame replies asynchronously.** `postMessage` is queued as a task, so a test that asserts
  synchronously after dispatching a message reads the state from before it was handled and passes
  vacuously. `frameFind.test.ts` awaits a macrotask after every post.

## Traps — platform detection

- **A corpus labelled by the markers the detector reads scores the detector against itself.** The
  detection labels are asserted by a person from knowledge of each project, and the fetch script
  cross-checks them against the page's own `<meta name="generator">` and *reports disagreements for
  a human* rather than trusting either side. That check caught eight on its first run.
- **The label answers "which scraper handles this", not "which program emitted the HTML".** They
  come apart: six sites now report `zensical` and still emit Material-for-MkDocs markup, so they
  stay `mkdocs`. Two had genuinely migrated (Astro, Rspress) and were relabelled `generic`.
- **Detection must be allowed to say "I don't know".** `(Generic, 1.0)` — full confidence in the
  fallback — makes "no idea" indistinguishable from "certain". The fallback sits well below
  `AUTO_ACCEPT`, and the metric that matters most is *confidently wrong*, gated with no margin: an
  unsure answer costs one dialog, a confident wrong one crawls a site with the wrong scraper.
- **Not every platform can be sampled.** GitBook is a hosted product now and its public instances
  are companies' own documentation under no redistributable licence, so it has zero fixtures and
  the matrix prints an empty row. That is the honest reading — a score for a class with one sample
  would be worse than none. `Platform::parse` returns `None` for an unknown label rather than
  falling back to `Generic`, or a typo would quietly inflate the non-documentation count.
- **A corpus that is mostly one class scores well for a detector that knows nothing.** The harness
  asserts the largest label stays under half the fixtures, for the same reason the relevance corpus
  carries a document floor.

- **The domain says who serves the pages, not what made them.** P2-014's first sketched rule is
  `url.contains("readthedocs") → ReadTheDocs, 0.95`, and the corpus holds
  `mkdocs-macros-plugin.readthedocs.io`, which is MkDocs. Detection matches HTML markers, never
  hostnames.
- **A book about a generator is full of that generator's name.**
  `doc.rust-lang.org/rustdoc/` is the mdBook-built rustdoc book, and
  `html.contains("rustdoc")` — the sketched rule — classifies it as rustdoc. The rustdoc rule
  therefore matches hashed asset paths (`/rustdoc-`, `normalize-`), and mdBook's furniture is
  checked **first**. Rule order in `detect::RULES` is load-bearing.
- **Some sites are unclassifiable from a homepage, and the right answer is to say so.**
  `docs.djangoproject.com` is Sphinx-built with a template carrying no Sphinx marker anywhere.
  It falls back to `Generic` at 0.4 — below `AUTO_ACCEPT` — and caps achievable accuracy at 127/128.
  Getting it "right" would need a rule that fired on prose, which is how confidently-wrong
  classifications start.

## Traps — scrapers and man pages

- **Most of the per-platform scraping was already in the generic path.** S1-8's furniture rules
  were each added to fix a leak in a corpus spanning six platforms, so they are cross-platform by
  construction. Adding profiles for Sphinx, rustdoc and mdBook changed **four golden files, all
  rustdoc**. Before writing a platform scraper, measure what the generic path already does — the
  answer was "nearly all of it".
- **A profile matches exact class tokens; the generic list matches substrings.** That is the whole
  reason profiles exist separately: `src` as a substring hits `srcset` and `source-code`, but as an
  exact token on a page known to be rustdoc it is precisely the source-link furniture.
- **mdBook's `header` class is on the anchor *inside* each heading**, so dropping it deletes every
  heading on the page. The golden corpus caught it on the first run. rustdoc's `toggle` is the same
  trap: `<details class="toggle top-doc">` is where the documentation lives.
- **The Sphinx and mdBook profiles are unmeasured.** Nothing in the 26-page corpus exercises them.
  They encode real markup those generators emit, but "kept" is not "verified".
- **macOS `mandoc` has no `-Q`.** The flag exists in OpenBSD's build; passing it made *every* page
  render empty, because mandoc printed usage to stderr and exited without reading anything. The
  exit code is not a useful signal either — mandoc exits non-zero for warnings, which shipped pages
  routinely have — so empty output is the test.
- **mandoc hard-wraps its HTML.** `<span class="Nd">make\n    gadgets</span>` is what a two-word
  description looks like, so anything comparing against it must collapse whitespace first.
- **Cross-references are linked only to pages that were discovered.** mandoc emits
  `<a class="Xr">ctype(3)</a>` with no `href` because it cannot know where the target lives. A link
  to a page the user has not installed looks like it would work and does not, which is worse than
  the plain text.
- **Man ingest runs external programs**, and they are invoked by absolute path with an argument
  vector, never through a shell. `PATH` is the user's shell's to control; a documentation reader
  resolving an executable through it is a documentation reader running arbitrary code.

- **A committed timing baseline is unsound, unlike every other baseline here.** Relevance and
  detection both commit one and gate on regression; timings cannot, because a figure recorded on
  one laptop fails on a slower one and passes on a faster one *while hiding a real regression*, and
  the failure reads as a code bug rather than a difference in the machine. The search benchmark
  gates on an **absolute** threshold with ~600× headroom instead: it catches a lost index or an
  accidental full scan, and deliberately nothing subtler.
- **Benchmark with the eval set's queries, not generated ones.** A synthetic query has whatever
  selectivity its author gave it, and a benchmark over generated pages measures the generator's
  idea of a document.

## Invariants — agent access

- **`tome mcp` speaks the legacy `2025-11-25` handshake on purpose, not out of date.** MCP's
  current revision (2026-07-28) abolished the `initialize` handshake for per-request versioning,
  but Claude Code 2.1.220 — released *after* it — still opens with `initialize` and, per the
  spec's own compatibility matrix, has **no fall-forward** against a modern-only server. Adding
  the modern era later is additive (a dual-era server is explicitly permitted); replacing the
  legacy one is not. Measured in [`docs/spikes/008-mcp-protocol.md`](../docs/spikes/008-mcp-protocol.md).
- **`initialize` must echo the client's requested protocol version whenever we support it.**
  Answering with our own newest instead is a **silent drop**: the client sends nothing further,
  writes no error, and the tools simply never appear. There is no diagnostic anywhere. Measured.
- **A notification is never answered.** `notifications/initialized` arrives in every session, and
  replying to it is itself a protocol violation. The `id`-is-absent check in the serve loop is
  what enforces it.
- **The MCP tools are read-only, and that is a security boundary.** The documentation Tome
  ingests is untrusted text that agents read, so a prompt injection in a scraped page must not
  reach a write. `tome_bookmark` is absent rather than opt-in because no bookmark model exists
  yet; whoever adds the first write-capable tool owes the opt-in gate P4-013 specifies.
- **Loopback is not a trust boundary, so the API has no loopback bypass.** Every process on the
  machine and every page in the user's browser can originate a loopback request. The bearer token
  is required on every route but `GET /api/v1/status`; the token is compared through hashed,
  branch-free digests; and the server keeps only the hash after startup.
- **The API's middleware order is load-bearing**: host/origin guard → rate limit → CORS → auth.
  The guard first, so a rebound request never reaches the limiter. The limiter before auth, so
  token brute-force burns the same budget. **CORS before auth, because a browser preflight
  carries no `Authorization` header** — putting auth first 401s every preflight and the
  allowlist silently stops working. Axum applies layers innermost-first, so the `.layer()` calls
  read in reverse.
- **A non-allowlisted `Origin` is refused, not merely denied CORS headers.** Header absence hides
  the *response*; refusal also stops the request having effects, which is what `mode: 'no-cors'`
  is for. Confirmed in a real browser: a hostile page cannot read the API even holding a valid
  token, and its opaque request is refused 403 server-side.

## Traps — agent access

- **A tool result that is too large is not a transport error.** 500 KB survives the JSON-RPC
  frame; the client then diverts it to a file and hands the model a *filename*. The page is not
  in the answer and nothing failed loudly. That is why `tome_get_page` has a byte budget, and why
  truncation cuts at a **block boundary** — half a code fence renders the rest of the
  conversation as code.
- **A heading's anchor id is not always on the heading.** Sphinx puts it on a sectioning wrapper,
  which normalization reduces to a standalone `Node::Anchor` *before* a heading whose own `id` is
  `None`. Code that reads only `Heading.id` reports "this page has no section anchors at all" for
  every Sphinx page — which is most of them. Both shapes must resolve.
- **`claude plugin validate` is the only authority on plugin format.** P4-017 specified a
  `slash_commands:` YAML manifest with `handler:` fields; no such format exists. A real plugin is
  a directory with JSON at `.claude-plugin/plugin.json`, markdown commands under `commands/`, and
  standard MCP config in `.mcp.json`. Same class of error as the Unix-socket MCP transport.
- **A plugin's own MCP tools are scoped `mcp__plugin_<plugin>_<server>__<tool>`.** An
  `allowed-tools` entry written against the bare tool name silently never matches.
- **A Claude Code session has no terminal**, so every plugin command that mutates must pass
  `--yes` — and for `tome remove` that makes the assistant's own confirmation the only one there
  is.
- **Authored files must not live in a build-output directory.** The Claude Code plugin and the
  cask were committed under `dist/`, behind `dist/*` plus gitignore negations. That made them
  *tracked*, which looked like the whole problem — but `dist/` is Vite's `outDir`, and Vite
  **empties its output directory on every build**, so `npm run build` deleted them. It went
  unnoticed for two stages because nothing ran a frontend build between committing them and
  S4-9's first `tauri build`, which does. The negations were a fix for the visible half of a
  two-part mistake. They now live in `packaging/`, and `dist/` is gitignored outright.
- **A health check that edits the config it is checking is checking nothing.** The registry
  verifier's first version `sed`-ed `max_pages` into a copy — which verifies a file no user runs,
  and silently failed to cap rustdoc/mdbook/readthedocs sources at all, because those types carry
  no `max_pages:` line for a `sed` to find. A two-minute check became a full crawl of the Cargo
  Book. `tome pull --max-pages` is a *runtime* override for this reason.
- **Check `robots.txt` before choosing a registry URL, and let the fetch tell you.** The
  verification job's first run rejected `nodejs.org/docs/latest/api/`: that site disallows
  `/docs/` and explicitly allows `/api/`, so the obvious URL was the forbidden one and the
  correct one was a path segment away. No review would have caught it.
- **macOS ships bash 3.2**, which has no `mapfile`. A script using it dies with a bare "command
  not found" partway through, after printing a banner that suggests it was working.

## Traps — errors, logging and recovery

- **A `_ =>` arm in `Error::suggestion` is how the taxonomy rots.** It is now exhaustive, so
  adding a variant stops the build until someone decides what a person should do about it. The
  two variants with no suggestion (`BlockedByRobots`, `Io`) are named in the test's `NO_ACTION`
  list with reasons — an omission and a decision look identical otherwise.
- **Every command an error names must exist**, and `REAL_COMMANDS` in `error.rs` enforces it.
  P5-004's own technical notes suggested `tome debug rebuild-index` at a time when no such
  command existed, and left a comment saying error strings naming non-existent commands are worse
  than no suggestion. That comment is now a test.
- **Error messages are whole sentences**, which is why an interpolated detail goes in parentheses
  (`"The download failed ({message})."`) rather than after a colon at the end. Six messages were
  fragments when the audit first ran; nobody had noticed, because each one reads fine alone and
  the inconsistency is only visible across the set.
- **The logger must not create its directory at startup.** `tome search` on a machine that has
  pulled nothing must exit 0 and create no library, and a test asserts it — so `DailyFile` names
  a directory and creates it on the first event. A logger initialised eagerly turns every
  read-only command into one that writes.
- **One `write_all` per log event, or two processes interleave mid-line.** The app and the CLI
  share a library and can run at once; `O_APPEND` writes under a page are atomic, several small
  writes are not. `LogWriter` buffers the event and appends it on `Drop`, which is exactly one
  event because `MakeWriter` hands out a fresh writer per event.
- **`with_ansi(false)` when the log file shares a writer with stderr**, or escape codes end up in
  the file. Losing colour on the terminal is the cheaper half of that trade.
- **`tome debug check` must not repair.** A diagnostic that fixes things cannot be run twice to
  see whether the fix worked — which is why it calls `SearchEngine::open`, never
  `open_or_rebuild`.
- **An empty library is not a fault.** `check` reports a machine that has pulled nothing as
  healthy and exits 0; reporting it as broken sends people looking for a problem that is a first
  run, and breaks `tome debug check && tome pull --all`.
- **The index and the database can disagree, and nothing says so.** An interrupted pull leaves
  the database ahead of the index; search then misses pages that are on disk, silently. That
  comparison is the one check in `debug check` with no other symptom.

## Traps — onboarding and preferences

- **`ReaderFrame.settings` existed from S1-13 with no caller.** The frame has an opaque origin
  and cannot inherit the shell's cascade, so a preference applied only to `document` changes the
  chrome and leaves the page alone. `src/lib/appearance.ts` is the one place preferences become
  attributes and it is applied to both; a second mapping would drift invisibly.
- **A default is the *absence* of an attribute, never a value.** `data-theme="system"` would need
  a CSS rule duplicating the `prefers-color-scheme` query that already handles it, and
  `data-text-size="default"` one duplicating the root font size. Both are duplication that
  drifts, and a test asserts the attributes are `null`.
- **Appearance preferences are named steps, not free numbers.** `tokens.css` rescales the entire
  system off the root font size, so an arbitrary px value lets someone pick 13px and shrink the
  UI chrome along with the prose. `enumPreference` also rejects a stored value outside its set —
  otherwise a hand-edited store sets an attribute no rule matches, leaving the default theme with
  a preference that reads as changed, which looks like a broken theme rather than a rejected one.
- **"The library is empty" is not "this is a first run".** Someone who removes their last source
  has not become a first-time user. Onboarding is gated on a *dismissed* flag as well as on the
  source count.
- **The registry ships in the bundle, not over the network.** P5-006 requires onboarding to work
  offline, and a catalogue that must be downloaded before it can say "you are offline" cannot.
  `bundle.resources` in `tauri.conf.json` and `RESOURCE_DIR` in `onboarding.rs` must agree, which
  is why a failure there reports the resolved path rather than returning an empty list.
- **A pull is minutes of blocking work.** `install_registry_source` runs it on
  `spawn_blocking`; on the async runtime's worker it would stall every other command. The
  progress event carries the source id, because a second install can start before the first ends.
- **The crawl phase has no denominator.** The crawler does not know how many pages a site has
  until it has found them, and an invented total makes a progress bar that goes backwards.
- **The shortcuts panel lists only what is bound.** PRD Appendix C is the canonical list and
  covers features that do not exist; showing ⌘D for "bookmark page" teaches the user something
  false and they find out by pressing it. `src/lib/shortcuts.ts` is the implemented subset, and a
  test asserts ⌘D is absent from it.

## Traps — the menu bar and the global shortcut

- **A registered global shortcut is not a working one.** Measured in SPIKE-001: registering
  `CmdOrCtrl+Space` — Spotlight's — **succeeds**, and the handler never fires, because macOS
  consumes the keystroke before any application sees it. `RegisterEventHotKey` refuses a
  combination held by another *application's* hotkey and not one held by the system, and no API
  lists either. Conflict detection is therefore two-sided: report the registration error, *and*
  refuse the reserved list in `src/lib/accelerator.ts`.
- **Require at least two modifiers.** A global `⌘K` overrides the frontmost application's own
  `⌘K` in every app for as long as Tome is running.
- **Read letters from `event.code`, not `event.key`.** With Alt held, macOS reports `key` as the
  composed character — Alt+D is `∂` — and Tauri's accelerator parser cannot use it. Shifted
  digits arrive as their symbols for the same reason.
- **`unregister_all` before registering a replacement**, or both combinations stay live and the
  user has no way to discover why.
- **Filter on `ShortcutState::Pressed`.** Without it the handler runs on press *and* release, so
  the window is raised twice per keystroke.
- **`show()` before `set_focus()`.** A hidden window cannot take focus, and the focus call
  silently does nothing.
- **Act on mouse *up* for the tray icon.** On mouse down, dragging the item along the menu bar to
  reposition it also opens the app.
- **The tray icon must be a template image** — black plus alpha, `icon_as_template(true)`. macOS
  recolours it for light, dark and highlighted; an icon with colour is invisible in one of them.
- **There is no Swift.** Tauri's `tray-icon` feature is `NSStatusItem`. Any plan, ticket or
  comment that describes an `AppDelegate`, an `NSPopover` or a Swift plugin is describing an
  architecture this repository does not have.

## Traps — performance and accessibility

- **`aria-modal="true"` does nothing to the keyboard.** It tells assistive technology the rest of
  the page is inert; Tab still walks straight out of the dialog and behind the overlay, where the
  focus ring is invisible and the next Return activates something the user cannot see. Both modals
  use `trapFocus`, which intercepts only the two edges and leaves the middle to the browser.
- **The startup budget is missed and there is nothing to optimise.** Measured: 625 ms median,
  of which **10 ms** is Tome's code. The rest is Tauri and WKWebView creation, and a debug build
  measures the same. Do not go looking for a Rust hot spot; there isn't one.
- **The syntax-set warm-up costs 0 ms.** Its comment claimed for two stages that it kept "several
  megabytes of inflated syntax dumps" off the first page view. syntect's bundled defaults are
  lump data that is not parsed at load. Measure before believing a comment about cost.
- **`ps -o rss` does not see the webview.** WKWebView runs in its own processes, so the 118 MB
  idle figure is the app process only and the real total is higher. Attributing them needs
  `footprint(8)` or Instruments.
- **`list_pages` returns every page of a source**, and the backend is fine with it — 19 ms and
  1 ms for 20 000 pages. The DOM is not. The sidebar renders a 200-row window.
- **`scripts/verify-bundle.sh` must be told which bundle to verify.** Its first version preferred
  `target/release`, so a tree holding both would verify a bundle nobody had just built and then
  fail the same-build digest check against the sidecar staged for the other one — a false
  negative that reads exactly like a real defect. `check.sh` now passes the path explicitly.
- **`${arr[-1]}` is bash 4.** macOS ships 3.2. Same family as `mapfile`; use
  `${arr[${#arr[@]}-1]}`.

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
- `brew style` **refuses to lint a cask outside a tap.** `scripts/check-cask.sh` stages a
  throwaway tap under `$(brew --repository)/Library/Taps` and removes it on exit — without that,
  "the cask is linted" is a claim nobody has ever checked. The CHANGELOG asserted it for two
  stages while the file it named did not exist.
- The built app is `adhoc, linker-signed`. `spctl` rejects it. Expected.
- **The CLI ships as a Tauri `externalBin` sidecar, and that is load-bearing.** Tauri wants
  `src-tauri/binaries/tome-<target-triple>` and copies it to `Contents/MacOS/tome`, dropping the
  triple. Three ways it goes wrong silently: the triple must match `--target`, not the *host*, or
  the bundler never finds it (`TOME_CLI_TARGET`); `cargo build -p tome-app` **fails** without it
  staged, so the sidecar step comes before every compile of the app; and `tauri build` will
  happily bundle a sidecar staged from an older tree, which is why `scripts/verify-bundle.sh`
  compares digests rather than checking the file exists.
- **`tauri.conf.json` deliberately has no `version` key.** Omitted, Tauri takes the version from
  the `src-tauri` crate, so `CFBundleShortVersionString` and `tome --version` cannot disagree —
  and they are the same number for the same reason the app and the CLI are the same build.
  `scripts/set-version.sh` is the only thing that writes it, and it must also bump the
  `version = "…"` on the **path dependencies** in `[workspace.dependencies]`, or the workspace
  stops resolving (`failed to select a version for the requirement tome-testkit = "^0.0.0"`).
- **`brew uninstall --zap` cannot remove a Keychain item**, so the API token outlives the
  uninstall. `tome config forget-token` exists for that, and the cask's caveats name it. Anything
  Tome stores outside the filesystem needs the same treatment.
- **Zap lists rot silently.** Every path in the cask was observed on a machine that had run Tome,
  and `scripts/verify-bundle.sh` re-derives the two that matter from `tome status --json` at
  verify time. Paths that no version has ever created are deliberately absent and say so in a
  comment — the iCloud container is listed nowhere, because sync does not exist.

## Traps — cargo-deny

- The graph is scoped to `aarch64-apple-darwin`, which excludes Tauri's Linux GTK backend.
- Unmaintained advisories are listed **individually, with dated reasons**, deliberately not
  `unmaintained = "warn"`, so a *new* one still fails the build. It has caught one already
  (`bincode` 1.3.3, arriving with syntect, RUSTSEC-2025-0141).
- **Tantivy's tree added no new ignores** — it passed clean on first run.
