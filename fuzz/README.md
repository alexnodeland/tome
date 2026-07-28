# Fuzz targets

Implementation-plan **S0-8**. The scaffolding lands now; targets are added
per-module as the modules land, because a fuzz harness introduced after the
parser is a harness nobody retrofits.

## Running

`cargo-fuzz` needs a nightly toolchain — `libfuzzer-sys` compiles with sanitizer
instrumentation that stable does not expose. The repository pins stable in
`rust-toolchain.toml`, so nightly is named explicitly:

```bash
cargo install cargo-fuzz          # once
rustup toolchain install nightly  # once

cargo +nightly fuzz list
cargo +nightly fuzz run paths -- -max_total_time=300
```

A crash is written to `fuzz/artifacts/<target>/`. Reproduce and minimise it:

```bash
cargo +nightly fuzz run paths fuzz/artifacts/paths/crash-<hash>
cargo +nightly fuzz tmin paths fuzz/artifacts/paths/crash-<hash>
```

**Commit the minimised input as a unit test**, not as a corpus entry. A corpus
file records that the case was once explored; a test records that it must never
regress.

`./scripts/check.sh` type-checks the targets (fast, stable toolchain) but does
not fuzz — fuzzing is unbounded by nature and belongs in a scheduled run, not
in a pre-commit gate.

## Targets

| Target | Property | Status |
|--------|----------|--------|
| `paths` | No `Paths` accessor panics on any input; every constructible `SourceId` resolves to exactly one component under the right root | Live |
| `model_ids` | `SourceId`/`PagePath`/`ContentHash` never panic, and whatever constructs cannot traverse (no separators, no dot segments) | Live |
| `html_parser` | Zero panics parsing any byte sequence (P1-012's criterion); root always a Document; AST serde round-trips; links http(s)-only | Live |
| `sanitizer` | No input yields a link/image URL with a script-capable scheme or an unsafe id/class token; output is idempotent and frozen-shape valid | Live |
| `source_config` | No panic on any YAML; whatever parses honours the validated invariants (rate ≤ cap, https-or-allow_insecure, …) | Live |
| `robots` | robots.txt parser/matcher never panics and never goes exponential; `/robots.txt` always fetchable | Live |
| `ssrf` | Address classifier is total; every v6 spelling of a v4 address classifies identically (no v4-mapped bypass); policy is monotone | Live |
| `annotation_anchor` | Re-anchoring resolves or reports `orphaned`, never lands on the wrong text | S3 |

The properties in the right-hand column are the point. A fuzz target that only
asserts "did not panic" catches crashes; one that asserts an invariant catches
defects. Where a real invariant exists, assert it.

## Corpus

Seed corpora are not committed. `fuzz/corpus/` and `fuzz/artifacts/` are
ignored: for the HTML targets the natural seed corpus is
`crates/tome-testkit/fixtures/` plus the golden corpus inputs, which are
already in the repository and can be pointed at directly:

```bash
cargo +nightly fuzz run html_parser crates/tome-testkit/fixtures -- -max_total_time=300
```
