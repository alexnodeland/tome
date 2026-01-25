# Examples Directory

Example configurations and reference implementations.

## Source Configurations

The `sources/` directory contains example YAML configurations for adding documentation sources to Tome.

### Available Examples

| File | Description |
|------|-------------|
| `rust-std.yaml` | Rust Standard Library (rustdoc format) |
| `python-3.yaml` | Python 3 documentation (Sphinx/ReadTheDocs format) |
| `generic-example.yaml` | Template for any web documentation (generic scraper) |

### Using Examples

1. Copy the example to your Tome config directory:
   ```bash
   cp examples/sources/rust-std.yaml ~/.tome/sources/
   ```

2. Customize as needed (change URL, version, etc.)

3. Open Tome - the source will appear in your library

### Configuration Reference

See the full configuration specification in:
- `.claude/plans/01-phase-1-foundation.md` (Ticket P1-008)
- `CLAUDE.md` (Configuration section)

## Creating Your Own Configurations

1. Start with `generic-example.yaml` as a template
2. Identify the documentation site's structure:
   - Where is the main content? (content_selector)
   - Where is the page title? (title_selector)
   - Where is the navigation? (nav_selector)
3. Test with a small max_depth first
4. Adjust selectors as needed

## Common Patterns

### Sphinx/ReadTheDocs
```yaml
source:
  type: readthedocs
  url: https://docs.example.com/en/latest/
```

### rustdoc
```yaml
source:
  type: rustdoc
  url: https://docs.rs/crate-name/latest/
```

### mdBook
```yaml
source:
  type: mdbook
  url: https://example.com/book/
```

### Generic (any site)
```yaml
source:
  type: generic
  url: https://example.com/docs/
  generic:
    content_selector: "main"
    title_selector: "h1"
    max_depth: 3
```
