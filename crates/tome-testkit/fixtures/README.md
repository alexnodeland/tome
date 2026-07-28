# Fixture sites

Miniature documentation sites, served offline by
[`FixtureServer`](../src/server.rs) (implementation-plan S0-6). One directory
per site; the directory name is what `FixtureServer::start("…")` takes.

| Site | Shape it imitates | Exercises |
|------|-------------------|-----------|
| `sphinx-example` | Sphinx / ReadTheDocs HTML output | Nested pages, `id` anchors and permalinks, `<pre>` code blocks, a local image, a stylesheet, `searchindex.js`, `robots.txt` with a disallowed directory |

## Rules for adding one

**Hand-author it. Do not copy a real site's HTML.** Only the *shape* is needed
— the element structure, class names, and anchor conventions a scraper keys
off. Copying pages in means committing someone else's licensed content to this
repository, and the legal posture question is still open (SPIKE-010).

Keep them small. A fixture is read by a person deciding whether a scraper is
correct; a 400 KB page defeats that. Everything a test needs to assert should
be visible in one screen of HTML.

Each site should contain at least one of each thing the pipeline has to get
right: a heading with an `id` (anchors must survive sanitization), a relative
link, an absolute external link, a local asset, and a page that is *not*
reachable from the index (so crawl scope is testable).

Real pages from real sites do belong in the **golden corpus** rather than here,
where they are inputs to a reviewable diff instead of served content.
