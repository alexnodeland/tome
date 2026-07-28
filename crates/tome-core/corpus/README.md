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

Real pages are third-party content. Keep each case to the single page that
demonstrates the structure, and note the source URL and retrieval date in
`input/SOURCES.md` — the legal posture for redistributing fetched pages is
SPIKE-010 and is not yet settled.
