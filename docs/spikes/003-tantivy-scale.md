# SPIKE-003 — Tantivy at documentation scale, measured

**Date:** 2026-07-29 · **Status:** complete · **Verdict:** all four criteria pass, with margin.
**Spec:** [`docs/plans/07-technical-spikes.md`](../plans/07-technical-spikes.md) § SPIKE-003.
**Gates:** Stage 2 ([`docs/plans/18`](../plans/18-implementation-plan.md) § Stage 2 entry gate).

## Result against the spec's success criteria

| Criterion | Budget | Measured | Margin |
|---|---|---|---|
| Peak RSS indexing 100k pages | < 1 GB | **439 MB** (worst case measured) | 2.3× |
| Search latency at 100k pages | < 100 ms | **18.7 ms** p95 worst shape; **< 2 ms** for everything else | 5× |
| Index size for 100k pages | < 500 MB | **224 MB** (pessimistic vocabulary) | 2.2× |
| Idle memory, index not loaded | < 50 MB | **2 MB** linked-only, **3 MB** with the index open | 16× |

**Proceed with Tantivy.** No sharding, no index-per-source, no memory-budget gymnastics — the
fallbacks the spike spec listed are all unnecessary.

## Method

`spikes/003-tantivy-scale/` — a standalone crate in **its own workspace**, excluded from the root
one, so that answering "should we adopt Tantivy?" does not put Tantivy into the product build
first. (S2-2 is where it enters `tome-core` properly, with a `cargo deny` review of its tree. The
harness comes out then; this document is the part that stays.)

**Memory is measured from outside the process.** Each phase is a separate subcommand run under
`/usr/bin/time -l`, whose "maximum resident set size" comes from the kernel. Reading `getrusage`
in-process would need `unsafe`, and a program reporting its own peak memory is the measurement
most likely to be quietly wrong.

The schema is the one **P2-002** specifies — `title`/`path`/`source_id` stored, `headers`/`body`
indexed at different boosts, a separate `code` field, a `category` facet — so the numbers describe
what will actually ship rather than a toy.

**Environment:** macOS 26.5, Apple M3 Max, 128 GB RAM, arm64, release build, tantivy 0.26.1.

### The corpus is synthetic, and that matters for exactly one number

100 000 real documentation pages is ~600 MB of HTML that cannot be committed or politely fetched.
The generator produces documentation-*shaped* text: title, headings, prose, and code blocks, with
a long-tailed size distribution averaging ~6 KB of body — which is what the 26 real pages in
`crates/tome-core/corpus` average.

Synthetic text flatters exactly one measurement: **index size**, because a small vocabulary
compresses far better than English plus identifiers. So rather than caveat it, the run measures
it. `--vocab N` mixes a long tail of `N` rare terms into 30% of word positions, drawn
**uniformly** — which is *worse* than the Zipfian distribution real text has, so the resulting
number is an upper bound.

## Raw output

Full capture in the run log; the lines that matter:

```
# 2026-07-29T01:14:58Z  Darwin arm64  Apple M3 Max   tantivy 0.26

RESULT rss   label=idle (no index opened)              peak_mb=2

RESULT index pages=10000  budget_mb=128 text_mb=61  total_s=0.4 pages_per_s=22585
RESULT rss   label=index 10000 pages (128MB budget)   peak_mb=186
RESULT disk  pages=10000  index_mb=8
RESULT rss   label=open 10000-page index              peak_mb=3

RESULT index pages=50000  budget_mb=128 text_mb=305 total_s=2.7 pages_per_s=18594
RESULT rss   label=index 50000 pages (128MB budget)   peak_mb=277
RESULT disk  pages=50000  index_mb=43

RESULT index pages=100000 budget_mb=128 text_mb=610 total_s=5.0 pages_per_s=19877
RESULT rss   label=index 100000 pages (128MB budget)  peak_mb=314
RESULT disk  pages=100000 index_mb=84
RESULT rss   label=open 100000-page index             peak_mb=3
RESULT rss   label=search 100000 pages (peak)         peak_mb=15

# search at 100k, 200 rounds each
segments=26 docs=100000
"single common term"   p50=0.70  p95=0.73  max=0.79   cold=0.94
"single rare term"     p50=0.02  p95=0.03  max=0.03   cold=0.08
"two terms"            p50=1.06  p95=1.12  max=1.22   cold=1.11
"phrase"               p50=18.16 p95=18.66 max=18.88  cold=18.63
"code-only identifier" p50=0.34  p95=0.36  max=0.39   cold=0.38
"four terms"           p50=1.81  p95=1.88  max=1.91   cold=1.94
"prefix"               p50=0.34  p95=0.35  max=0.36   cold=0.37
fetch_stored docs=20 total_ms=0.16

# writer memory budget sweep, 50k pages
budget= 50MB  peak_mb=235  index_mb=52  pages_per_s=20247
budget=128MB  peak_mb=263  index_mb=46  pages_per_s=19156
budget=512MB  peak_mb=247  index_mb=40  pages_per_s=54016

# incremental
RESULT index pages=1000 start_at=100000  total_s=0.5
RESULT rss   label=incremental +1k onto 100k          peak_mb=72

# vocabulary sweep at 100k pages — the index-size caveat, measured
vocab=  5000  index_mb=169  peak_mb=354  segments=47   pages_per_s=17297
vocab= 50000  index_mb=212  peak_mb=424  segments=71   pages_per_s=10386
vocab=200000  index_mb=224  peak_mb=439  segments=115  pages_per_s=4858
```

## Findings

**1. Indexing is not the bottleneck. Crawling is, by three orders of magnitude.**
100 000 pages index in 5–21 seconds. Crawling 100 000 pages at the ≤ 4 req/s cap SPIKE-010 derived
takes **about seven hours**. This reframes S2-3 (incremental indexing): its value is *not* saving
indexing time, which is free. Its value is not having to re-crawl. Anything in S2-3 that trades
crawl work for index work is a good trade; anything that adds complexity purely to avoid
re-indexing is not.

**2. Index size scales with vocabulary and then plateaus — plan for ~2.2 KB per page.**
84 MB with the toy vocabulary, 212 MB at 50 000 distinct terms, 224 MB at 200 000. It flattens
because tail terms have tiny posting lists: the dictionary grows, the postings do not. Even the
pessimistic uniform-tail figure is under half the 500 MB budget, so **use ~220 MB per 100k pages**
as the planning number and stop worrying about it.

**3. The writer's memory budget is not a dial for peak RSS.** 50 MB → 235 MB peak, 128 MB →
263 MB, 512 MB → 247 MB. Peak is dominated by per-thread arenas and merge activity, not the
nominal budget. And the 512 MB budget was **2.7× faster** (54k vs 19k pages/s) at no memory cost,
because it flushes fewer, larger segments.
*Recommendation for S2-2:* use a large writer budget (512 MB) for bulk indexing. It buys speed and
does not cost proportional memory — the opposite of what the parameter's name suggests.

**4. Segment count is the thing that actually degrades search, and it grows with vocabulary.**
26 segments at the toy vocabulary, 115 at 200k terms. Common-term p95 tracked it: 0.73 ms →
1.30 ms. Everything stayed trivial here, but the *mechanism* is worth knowing — search cost is
roughly linear in segment count, and a library that syncs incrementally forever will drift toward
many small segments.
*Recommendation for S2-3:* set an explicit merge policy rather than inheriting the default, and
make segment count something the benchmark in S2-12 watches.

**5. Phrase queries cost 15–20× a term query and are the only shape in double-digit milliseconds.**
18.7 ms p95 at 100k, against 0.73 ms for a single common term. Still 5× inside budget, but phrase
search is where the headroom goes. If S2-7's UI ever issues a phrase query per keystroke, that is
the thing to debounce.

**6. Idle memory is 3 MB, but the honest framing is that the process does not own the index.**
Tantivy mmaps its files, so RSS reflects only the pages actually touched. The `< 50 MB` criterion
passes at 3 MB, but the index still occupies OS page cache, and on a memory-pressured machine that
cache is evicted and searches fault it back in. The right claim is "Tome does not hold the index in
its own heap", not "the index is free".

**7. Cold and warm search differ by less than a millisecond**, and only for the first query
(0.94 ms cold vs 0.70 ms p50 on a common term). There is no meaningful warm-up period to design
around — no need to pre-warm the searcher at launch.

**8. Incremental indexing is cheap.** Adding 1 000 pages to a 100 000-page index: 0.5 s, 72 MB
peak. Roughly a fifth of the memory of a bulk build and effectively instant, which is what a sync
that finds a handful of changed pages will actually do.

## What Stage 2 inherits

- **Proceed with Tantivy.** No sharding by source, no index-per-source, no lazy-loading scheme.
  The fallbacks in the spike spec are all unnecessary at this scale.
- **Writer budget 512 MB for bulk indexing**, and treat the parameter as a speed knob rather than
  a memory one (finding 3).
- **An explicit merge policy** (finding 4), with segment count in the S2-12 benchmark.
- **The planning number is ~220 MB of index per 100 000 pages**, ~2.2 KB per page (finding 2).
- **Budget the latency headroom for phrase queries** (finding 5); everything else is sub-2 ms.
- **S2-3's justification is avoiding re-crawls, not avoiding re-indexing** (finding 1). That should
  shape what the ticket actually builds.

## What this spike did not measure

- **Relevance.** Nothing here says whether the results are any *good* — that is S2-1's eval set,
  and the plan is explicit that it comes before any ranking work.
- **The real code tokenizer.** The `code` field is indexed with the default tokenizer; P2-002's
  camelCase/snake_case tokenizer will change term counts and therefore index size somewhat. The
  vocabulary sweep bounds that effect: even a 200 000-term vocabulary lands at 224 MB.
- **Concurrent search during indexing.** Searching while a sync writes is a real scenario and is
  untested here.
- **A memory-pressured machine.** Everything was measured on a 128 GB host with the whole index
  comfortably in page cache. Finding 6 is the caveat; the behaviour under pressure is unknown.
