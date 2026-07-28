# Contributing to Tome

Thanks for looking. **Tome is pre-implementation** — there is no code yet, so the useful
contributions right now are different from a normal project's.

## Right now, the most valuable contributions are

1. **Telling us the plan is wrong.** Open a GitHub Discussion. Disagreement about architecture,
   scope, or whether this should be built at all is more valuable than agreement. The
   [plan review](docs/reviews/2026-07-28-plan-review.md) is itself an example of what this looks
   like.
2. **Running a spike.** [`docs/plans/07-technical-spikes.md`](docs/plans/07-technical-spikes.md)
   lists eleven time-boxed questions, each one to three days, each blocking a phase. Claim one in
   an issue, run it, and open a PR with the findings and a prototype under `spikes/`.
3. **Answering an open decision** in [`docs/decisions/`](docs/decisions/).

## Once implementation starts

### Setup

```bash
# Prerequisites: macOS 12+ on Apple Silicon, Xcode Command Line Tools, Homebrew
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
brew install node@20

git clone git@github.com:alexnodeland/tome.git && cd tome
npm install && cargo build
npm run tauri dev
```

### Before you open a PR

```bash
./scripts/check.sh          # everything CI runs; --fast skips the app build
```

GitHub Actions is unavailable while the repository is private, so this script
**is** the gate rather than a convenience. It runs exactly what
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs; change one and change the other in the
same commit.

### Test infrastructure

| | |
|---|---|
| Fixture HTTP server | [`crates/tome-testkit/src/server.rs`](crates/tome-testkit/src/server.rs) — serves committed doc-site fixtures offline, scripts failures, and shuts down mid-test so offline behaviour is assertable |
| Golden corpora | [`crates/tome-testkit/src/golden.rs`](crates/tome-testkit/src/golden.rs), corpora under [`crates/tome-core/corpus/`](crates/tome-core/corpus/README.md) |
| Property tests | `proptest`, alongside the module they cover |
| Fuzz targets | [`fuzz/`](fuzz/README.md) — its own workspace, needs nightly to run |

```bash
TOME_UPDATE_GOLDEN=1 cargo test -p tome-core normalization   # rewrite goldens...
git diff -- crates/tome-core/corpus                          # ...then review the diff
cargo +nightly fuzz run paths -- -max_total_time=300
```

Update mode deliberately fails the run it changes anything in; the passing run is the one after the
diff has been read. When output is judged rather than asserted — normalization, rendering,
snippets — add a corpus case, not a hand-written `assert_eq!` on a fragment of HTML.

### Standards

- **Conventional commits** (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`), lowercase
  subject, 72 characters or fewer.
- **Tests for new behaviour.** Per-module coverage thresholds are in
  [`docs/plans/08-testing-strategy.md`](docs/plans/08-testing-strategy.md); new code is held to a
  higher bar than the existing average.
- **Specification and code change together.** If your PR adds an HTTP route, an MCP tool, a CLI
  command, or a config key, add it to its specification document in the same PR. Several commands
  entered the plan only as examples in unrelated documents and were never specified — that is how
  a CLI surface stops being knowable.
- **Link, don't restate.** Each shared fact has one owning document (see the ownership table in
  [`00-project-overview.md`](docs/plans/00-project-overview.md)). Copying a table into a second
  document is how this plan previously ended up with three different critical paths.
- **No `unsafe` without a comment justifying it.** No `unwrap()` on anything derived from network
  input, file contents, or another device's data.

### Contributing a source to the registry

A registry entry is one YAML file describing how to scrape a documentation site. It contains
**configuration only, never content** — Tome does not host anyone's documentation.

1. Add `registry/sources/<id>.yaml`
2. Verify locally: `tome add --config registry/sources/<id>.yaml && tome pull <id>`
3. Confirm the site's `robots.txt` and terms permit automated access. If they do not, the source
   does not go in the registry — users may still add it manually on their own machine, which is a
   different act.
4. Include `attribution.homepage` and `attribution.licence`.

CI re-verifies every registry entry weekly against the live site and opens an issue when one breaks.

## Code of conduct

Be decent. Assume good faith. Critique work, not people — including when the work is this plan.

## Security

Do not open a public issue for a vulnerability. See [`SECURITY.md`](SECURITY.md).

## Licensing of contributions

Tome is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at the user's
option. Unless you state otherwise, contributions you submit are dual-licensed on the same terms.

Contributions are made under the [Developer Certificate of Origin](https://developercertificate.org/);
sign off with `git commit -s`.
