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
| `normalization` | Normalized AST → HTML across ≥ 20 real sites spanning every target platform | S1-8 |
| `sanitizer` | The XSS payload corpus (nothing survives) **and** the anchor corpus (nothing breaks) | S1-9 |
| `snippets` | Search result snippet generation | S2 |

`normalization` is the one the product's viability actually rests on: it is
where "does documentation come out the other side looking right?" gets an
answer that can be reviewed rather than asserted.

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
