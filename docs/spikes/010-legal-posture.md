# SPIKE-010 — documentation scraping: legal and ToS posture

**Date:** 2026-07-28 · **Status:** complete · **Verdict:** the design is on the right side of
every line found; concrete rules below.
**Spec:** [`docs/plans/07-technical-spikes.md`](../plans/07-technical-spikes.md) § SPIKE-010.
**Feeds:** RISK-011 (`docs/plans/11-risk-register.md`), the corpus rules
(`crates/tome-core/corpus/README.md`), the S1-4 HTTP client spec, and the README.

**This is an engineering analysis, not legal advice.** Nothing found suggests a lawyer is needed
before Phase 1 (the spike's own trigger for escalation), but the owner should read the position
statement below before it ships in the README, and DEC-001-style sign-off applies to anything
that changes distribution posture.

The analysis keeps RISK-011's three-flavour distinction and answers each separately: **crawling
behaviour**, **local caching**, **redistribution**. Conflating them is how this goes wrong.

## What was actually checked

`robots.txt` of the ten most likely sources, fetched 2026-07-28 (raw capture in the session log;
re-fetch rather than trusting this table to stay true):

| Host | robots.txt says | Reading |
|---|---|---|
| docs.python.org | Disallow: `/dev`, EOL versions (`/2/`…`/3.9/`) | Current docs open; the disallows are SEO hygiene |
| doc.rust-lang.org | Disallow: old (`/1.*`, `/0.*`) and first/second-edition book | Current docs open |
| docs.rs | Disallow: semver-redirect URLs (`*/^`, `*/~`) only | Open; disallows are redirect dedup, documented inline |
| docs.readthedocs.io | Disallow: one hidden version; **links their automated-access guidelines** | Open, and crawling is explicitly welcomed (below) |
| developer.mozilla.org | Disallow: `/api/`, `/*/files/`, `/media` | Docs pages open |
| go.dev | `Allow: /` | Fully open |
| nodejs.org | **Disallow: `/docs/`**, Allow: `/api/`, `/dist/latest/docs/api/` | Crawl the current API docs at `/api/`, never the versioned `/docs/` tree |
| kubernetes.io | Disallow: `/legacy/`, v1.0/v1.1, 404 pages | Current docs open |
| docs.docker.com | Disallow: one path; `Content-Signal: ai-train=yes, search=yes, ai-input=yes` | Open, with explicit machine-access consent signal |
| docs.gitbook.com | `Allow: /`; same Content-Signal | Open |

**Not one of the ten forbids automated access to current documentation.** Every Disallow found is
version hygiene, redirect dedup, or non-content paths. Two hosts have adopted `Content-Signal`
and explicitly consent to search/AI-input use.

**Read the Docs publishes the rules Tome should treat as the industry's floor**
([automated-access guidelines](https://docs.readthedocs.com/platform/stable/automated-access.html),
fetched 2026-07-28): respectful crawlers are *welcomed*; "keep to under **4 requests per
second**"; archives at 1 request/second; "**identify yourself — put a domain or email in your
user agent**"; use cache headers and ETags; distributed traffic that evades rate limits "is far
more likely to result in a ban". Tome's planned behaviour (honest UA, conditional GET, per-host
rate limit) is exactly what they ask for. The P1-008 spec owns the client's numbers; this spike's
input to it: **default well under 4 req/s per host, and the UA must carry the project URL.**

Content licences of the same sources (fetched 2026-07-28):

| Source | Docs licence | Redistribution conditions |
|---|---|---|
| Python docs | PSF License v2 (code samples ≥3.8.6 also 0BSD) | Retain the PSF copyright notice; derivative works must summarize changes |
| MDN | CC-BY-SA 2.5+ | Attribution = document title + link + "Mozilla Contributors" + modifications noted; **ShareAlike** — derivatives carry the same licence |
| Rust official docs | MIT OR Apache-2.0 (dual, per rust-lang policy; verify per repo) | Standard permissive attribution |
| go.dev | CC-BY 4.0 | Attribution |
| kubernetes.io | CC BY 4.0 (site footer) | Attribution |
| Node API docs | MIT-style (repo LICENSE covers "software and associated documentation files") | Notice retention |
| docs.rs | **Per-crate** — the host makes no licence claim over hosted docs | Whatever the crate says; no blanket answer exists |
| ReadTheDocs-hosted | **Per-project** — RTD's guidelines say "content belongs to project owners" | Per-project |
| GitBook-hosted | **Per-author** | Per-site |
| Docker docs | Apache-2.0 (docs repo) | Notice retention |

## Precedent

**DevDocs is the cheapest available precedent and the most demanding one**, because DevDocs
*redistributes* — it bundles content and serves it. Its contribution bar (fetched 2026-07-28 from
their CONTRIBUTING.md): *"the documentation's license must permit alteration, redistribution and
commercial use, and the documented software must be released under an open source license"* —
and they refuse anything that doesn't clear it. Dash and Zeal sit on the docset model: content is
generated from official sources or fetched to the user's machine by the user's own action, which
is Tome's shape. The conclusion from precedent: **the redistribution bar is the only hard bar,
and Tome's runtime never crosses the line that makes it apply.** The one Tome artifact that does
cross it is the golden corpus (below).

## The position (drafted for the README)

1. **Tome is a personal cache.** It fetches pages the user chose, at the user's request, to the
   user's machine — the same pages their browser would fetch, kept so they work offline. Nothing
   is uploaded, shared, or served.
2. **Tome is a polite client.** It obeys `robots.txt` (non-overridable for registry-shipped
   configurations), sends an honest User-Agent naming the project, rate-limits per host, honours
   `Retry-After`, and revalidates with conditional requests instead of re-downloading.
3. **Tome redistributes configurations, never content.** The registry is scraper *configs* —
   URLs and CSS selectors. No documentation text ships with the app or the registry, ever.
4. **Every page keeps its provenance.** The reader shows the origin link and the upstream licence
   where determinable; exports carry them along.
5. **Removal on request.** A documentation owner who objects gets their registry entry removed —
   the process below, not a debate.

## Attribution rules (concrete enough to implement)

- Every stored page records: **origin URL, retrieval timestamp, and detected licence** (from
  page metadata, a `LICENSE` link, or the source config's `license` field; `unknown` is a valid
  value and is displayed as such). The reader header shows origin; the licence sits in page info.
- Exports (S4) must embed origin URL + retrieval date + licence in the output.
- Where a licence names an attribution format (MDN: title + link + "Mozilla Contributors"), the
  source config carries it and the reader/export uses it verbatim.

## The registry takedown policy

- The registry repo carries a `TAKEDOWN.md`: a documentation owner (or anyone acting for one)
  opens an issue titled `takedown: <source-id>`; the entry is removed in the next registry
  release, target **≤ 7 days**, no relitigating. Disputes about whether the requester speaks for
  the source are resolved in favour of removal.
- Registry review checks, per new entry: `robots.txt` permits the crawl paths; the host's ToS (if
  any) does not forbid automated access; the `license` field is filled or explicitly `unknown`.
- **No shipped opt-out list is needed today** — none of the ten sources forbids access, so there
  is nothing to put in it. The mechanism if one appears is registry exclusion, not app code.

## The corpus rule (this is what was blocking S1-8)

The golden corpus in this repository **is redistribution** — the repo is intended to go public.
Therefore the corpus applies the DevDocs bar, not the personal-cache bar:

- **Committed corpus inputs must come from sources whose licence permits alteration and
  redistribution** (PSF-2.0, CC-BY, CC-BY-SA, MIT, Apache-2.0 all qualify). Sources with
  per-project/unknown licences (arbitrary RTD subdomains, GitBook sites, docs.rs crates without
  a permissive docs licence) may be used in local, uncommitted testing only.
- Each committed page carries an entry in the suite's `input/SOURCES.md`: URL, retrieval date,
  licence, and the one-line modification note the PSF and CC licences ask for ("truncated to N
  bytes; scripts removed" and similar).
- CC-BY-SA inputs (MDN) are acceptable — the goldens derived from them inherit the licence, and
  `SOURCES.md` says so per suite. If keeping licence bookkeeping per-suite ever costs more than
  MDN adds, drop MDN from the corpus rather than the rule.
- This comfortably covers the ≥ 20-site normalization target: Python/Go/Kubernetes/Rust/Node and
  the long tail of Apache/MIT-licensed project docs span every v1.0 platform.

## Input to RISK-011

Probability drops from 4 to 2: all ten likely sources permit the planned behaviour, the strictest
host publishes numbers Tome already planned to beat, and the only redistribution artifact (the
corpus) now has a licence gate. Impact stays 4 — a mishandled complaint is still expensive. The
mitigations RISK-011 lists are confirmed as the right ones; nothing new to add beyond the
takedown SLA above.
