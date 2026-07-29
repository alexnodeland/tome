# Golden corpora

Committed inputs and their expected output, checked by the harness in
[`tome-testkit`](../../tome-testkit/src/golden.rs) (implementation-plan S0-7).

```text
corpus/<suite>/
├── input/     one file per case
└── golden/    the expected output, same file name
```

A suite is run from a test in this crate:

```rust
let report = Golden::new("corpus/normalization").check(|case| normalize(&case.text()))?;
assert!(report.is_ok(), "{report}");
```

## Workflow

```bash
cargo test -p tome-core normalization                      # see the diff
TOME_UPDATE_GOLDEN=1 cargo test -p tome-core normalization  # rewrite goldens
git diff -- crates/tome-core/corpus                        # the actual review
cargo test -p tome-core normalization                      # confirm
```

Update mode fails the run it changes anything in, by design. The passing run is
the one after the diff has been looked at.

## Suites

| Suite | Checks | Lands with |
|-------|--------|------------|
| `normalization` | Normalized AST across documentation pages. **26 cases spanning six platforms** — Sphinx, mdBook, rustdoc, Node, Hugo, go.dev — every one a real page, licences and provenance in `normalization/input/SOURCES.md` | S1-8 |
| `sanitizer` | The XSS payload corpus (nothing survives) **and** the anchor corpus (nothing breaks) | S1-9 |
| `snippets` | Search result snippet generation | S2 |
| `relevance` | **Search ranking quality.** 207 labelled queries over 339 real pages, scored as MRR and recall@1/3/10, with per-query rank deltas against a committed baseline | S2-1 |

`normalization` is the one the product's viability actually rests on: it is
where "does documentation come out the other side looking right?" gets an
answer that can be reviewed rather than asserted.

**`relevance` does not use the `input/`–`golden/` layout above.** Its directory
holds `corpus.yaml` (per-source metadata), `queries.yaml` (207 labelled
queries), `baseline.json` (the committed scores), and `pages/` — 339 documents,
each a serialized `store::StoredPage` exactly as `pipeline::pull` wrote it, so
the eval indexes what the product indexes. Provenance and licences for all 339
are in `pages/SOURCES.md`. Its workflow is `TOME_UPDATE_BASELINE=1`, mirroring
`TOME_UPDATE_GOLDEN`.

**The corpus size is load-bearing, not incidental.** It began at 26 documents
and at that size the gate could not discriminate at all — removing an entire
indexed field from the query moved MRR by 0.0036 and tripped nothing. At 339 it
is decisive on real damage (the same class of change moves MRR by 0.32) while
staying quiet on neutral ones. `tests/relevance.rs` records the measurements.
The Stage 2 exit gate now carries a ≥ 150 document floor for this reason, and
the harness asserts it.

## Adding cases

Inputs here are real pages, unlike the hand-authored sites in
`tome-testkit/fixtures/` — a normalization corpus made of pages written to be
easy to normalize proves nothing. Save the page as fetched, do not reformat it
(Prettier is configured to leave this directory alone), and name it
`<platform>-<what-is-interesting-about-it>.html`.

Real pages are third-party content, and committing them here is
**redistribution** — this repository is intended to go public. SPIKE-010
settled the rule ([`docs/spikes/010-legal-posture.md`](../../../docs/spikes/010-legal-posture.md)):

- Only pages whose licence permits alteration and redistribution may be
  committed (PSF-2.0, CC-BY, CC-BY-SA, MIT, Apache-2.0 qualify). Pages from
  sources with per-project or unknown licences stay local and uncommitted.
- Every committed case gets a line in the suite's `input/SOURCES.md`: URL,
  retrieval date, licence, and a one-line note of modifications ("truncated to
  N bytes; scripts removed") — the PSF and CC licences ask for exactly that.
- CC-BY-SA inputs (MDN) make the derived goldens CC-BY-SA too; the suite's
  `SOURCES.md` must say so. If that bookkeeping ever costs more than MDN adds,
  drop MDN from the corpus rather than the rule.

Keep each case to the single page that demonstrates the structure.
