# The Tome source registry

Ready-made source configurations, so a new user's first experience is picking
documentation rather than writing YAML.

**It contains configuration, never content.** Tome does not host or
redistribute anyone's documentation — a user's own machine fetches it from the
origin. That is the property that keeps the legal posture clean
([SPIKE-010](../docs/spikes/010-legal-posture.md), RISK-011), and it is the one
thing here that must never be traded for convenience.

## Layout

| | |
|---|---|
| `index.yaml` | What the app browses: id, name, category, homepage, licence, config path, and the date verification last passed |
| `sources/<id>.yaml` | The tested scraper config, in the same schema a user would write by hand (PRD Appendix A) |

The file name is the source id, as everywhere else in Tome.

## Adding a source

One PR containing one YAML file and one index entry. Both halves are checked:

```sh
cargo test -p tome-core --test registry     # offline; runs in the gate
./scripts/verify-registry.sh <id>           # live; run this before opening the PR
```

The offline tests enforce what is *always* an error — the config parses with
the real parser, the index and the config agree on name/category/licence/
homepage, ids are unique, every config file is indexed, and **`robots.txt` is
not disabled, the rate limit is not raised, and `allow_insecure` is not set**.
That last group matters more here than anywhere else in Tome: a shipped config
crawls someone else's server on behalf of every user who installs it.

The live script pulls 25 pages into a throwaway library and asks only "does
this scraper still find anything". Zero pages from a site that answered is what
scraper rot looks like.

**Check `robots.txt` before choosing a URL.** The registry's first run caught
this immediately: `nodejs.org/docs/` is `Disallow`ed while `nodejs.org/api/`
is explicitly `Allow`ed, so the obvious-looking URL was the forbidden one and
the correct URL was one path segment away. `robots.txt` is obeyed by default
and **is not overridable for registry-shipped configurations**.

## Verification

`scripts/verify-registry.sh` is the mitigation for RISK-003 (scraper rot). It
is **not** part of `./scripts/check.sh`, deliberately: a gate that fails when
someone else's website is down teaches everyone to ignore the gate. Run it on
a schedule, read the output, open an issue for what broke.

```sh
./scripts/verify-registry.sh                        # all sources
./scripts/verify-registry.sh rust-std               # one
TOME_VERIFY_UPDATE=1 ./scripts/verify-registry.sh   # write back `verified:` dates
```

Only passing sources get a new date. A failure leaves the **old** date visible,
because "last known good" is what someone triaging needs to know.

A stale `verified` date is the signal. It is the entire reason the field
exists: scraper rot is otherwise discovered by users.

## Status

Four sources, all verified 2026-07-30. The v1.0 target is 30, covering the
languages and frameworks at the top of the Stack Overflow survey
([PRD § Source Registry](../docs/PRD.md#11-source-registry)).
