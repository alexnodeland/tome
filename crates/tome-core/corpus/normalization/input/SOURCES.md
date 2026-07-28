# Normalization corpus — sources

Per `corpus/README.md`, every committed input records its URL, retrieval
date, licence, and modifications — the SPIKE-010 licence gate for the golden
corpus (which ships publicly, so redistribution rules apply).

## Current inputs

| File | Source | Retrieved | Licence | Modifications |
|------|--------|-----------|---------|---------------|
| `sphinx-index.html` | `tome-testkit/fixtures/sphinx-example/index.html` | — (repo-owned) | MIT OR Apache-2.0 (this repository) | none — hand-authored fixture |
| `sphinx-api-reference.html` | `tome-testkit/fixtures/sphinx-example/api/reference.html` | — (repo-owned) | MIT OR Apache-2.0 (this repository) | none — hand-authored fixture |
| `sphinx-guide.html` | `tome-testkit/fixtures/sphinx-example/guide/index.html` | — (repo-owned) | MIT OR Apache-2.0 (this repository) | none — hand-authored fixture |

## Status: seeded, not yet complete

These three are the repository's own hand-authored Sphinx fixture — no licence
question, and they exercise the one case the whole vertical slice rests on
(the `<dl>`-based API entry). They are a **starter suite**: the golden harness
and the pipeline are proven end to end, but S1-8's acceptance criterion is
normalization judged across **≥ 20 real sites spanning every target
platform**, and that expansion is deliberately deferred to keep this landing
licence-clean.

The real pages come next, each cleared against the gate in `corpus/README.md`:
Python (PSF-2.0), Go / Kubernetes (CC-BY-4.0), Rust std (MIT/Apache), Node
(MIT), and the long tail of permissively-licensed project docs — all confirmed
permissive by SPIKE-010 (`docs/spikes/010-legal-posture.md`). MDN (CC-BY-SA)
may be included, in which case the derived goldens inherit CC-BY-SA and this
file records it per that entry.
