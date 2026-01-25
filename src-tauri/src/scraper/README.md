# Scraper Directory

HTTP scraping and documentation fetching.

## What Belongs Here

- **HTTP client** configuration and utilities
- **Platform-specific scrapers** (ReadTheDocs, rustdoc, etc.)
- **Generic scraper** with configurable selectors
- **Crawling logic** (BFS, depth limits, URL filtering)
- **Rate limiting** and retry logic

## What Does NOT Belong Here

- HTML parsing/normalization (use `parser/`)
- Database operations (use `storage/`)
- Search indexing (use `search/`)
- Command handlers (use `commands/`)

## Module Structure

```
scraper/
├── mod.rs              # Module exports
├── client.rs           # HTTP client configuration
├── generic.rs          # Generic scraper with CSS selectors
├── readthedocs.rs      # ReadTheDocs/Sphinx scraper
├── rustdoc.rs          # rustdoc scraper
├── mdbook.rs           # mdBook scraper
├── crawler.rs          # BFS crawling logic
├── url_filter.rs       # URL include/exclude patterns
├── rate_limiter.rs     # Request rate limiting
└── tests.rs            # Unit tests
```

## Core Types

```rust
// mod.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a scrape operation
#[derive(Debug, Clone)]
pub struct ScrapeConfig {
    /// Entry point URLs to start crawling
    pub entry_points: Vec<String>,
    /// Maximum crawl depth from entry points
    pub max_depth: u32,
    /// Patterns to include (regex)
    pub include_patterns: Vec<String>,
    /// Patterns to exclude (regex)
    pub exclude_patterns: Vec<String>,
    /// CSS selector for main content
    pub content_selector: String,
    /// CSS selector for page title
    pub title_selector: String,
    /// CSS selector for navigation (optional)
    pub nav_selector: Option<String>,
    /// Maximum requests per second
    pub rate_limit: f32,
    /// Request timeout in seconds
    pub timeout_secs: u64,
}

impl Default for ScrapeConfig {
    fn default() -> Self {
        Self {
            entry_points: vec![],
            max_depth: 4,
            include_patterns: vec![],
            exclude_patterns: vec![],
            content_selector: "main, article, .content, #content".into(),
            title_selector: "h1, title".into(),
            nav_selector: None,
            rate_limit: 5.0,
            timeout_secs: 30,
        }
    }
}

/// Result of scraping a single page
#[derive(Debug, Clone, Serialize)]
pub struct ScrapedPage {
    pub url: String,
    pub path: String,
    pub title: String,
    pub html: String,
    pub links: Vec<String>,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}

/// Result of a complete crawl operation
#[derive(Debug)]
pub struct CrawlResult {
    pub pages: Vec<ScrapedPage>,
    pub errors: Vec<CrawlError>,
    pub stats: CrawlStats,
}

/// Statistics about the crawl
#[derive(Debug, Default)]
pub struct CrawlStats {
    pub pages_fetched: u32,
    pub pages_skipped: u32,
    pub bytes_downloaded: u64,
    pub duration_secs: f64,
}

/// Error during crawling
#[derive(Debug)]
pub struct CrawlError {
    pub url: String,
    pub error: ScrapeError,
}
```

## Scraper Pattern

```rust
// generic.rs
use crate::error::ScrapeError;
use reqwest::Client;
use scraper::{Html, Selector};

pub struct GenericScraper {
    client: Client,
    config: ScrapeConfig,
    rate_limiter: RateLimiter,
}

impl GenericScraper {
    pub fn new(config: ScrapeConfig) -> Result<Self, ScrapeError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .user_agent("Tome/1.0 (Documentation Reader)")
            .build()
            .map_err(ScrapeError::ClientError)?;

        let rate_limiter = RateLimiter::new(config.rate_limit);

        Ok(Self {
            client,
            config,
            rate_limiter,
        })
    }

    /// Fetch a single page
    pub async fn fetch_page(&self, url: &str) -> Result<ScrapedPage, ScrapeError> {
        // Wait for rate limiter
        self.rate_limiter.acquire().await;

        // Fetch HTML
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(ScrapeError::NetworkError)?;

        if !response.status().is_success() {
            return Err(ScrapeError::HttpError(response.status()));
        }

        let html = response
            .text()
            .await
            .map_err(ScrapeError::NetworkError)?;

        // Parse and extract
        let document = Html::parse_document(&html);
        let title = self.extract_title(&document)?;
        let links = self.extract_links(&document, url);

        Ok(ScrapedPage {
            url: url.to_string(),
            path: extract_path(url),
            title,
            html,
            links,
            fetched_at: chrono::Utc::now(),
        })
    }

    /// Crawl starting from entry points
    pub async fn crawl(&self) -> Result<CrawlResult, ScrapeError> {
        let mut crawler = Crawler::new(&self.config);

        for entry in &self.config.entry_points {
            crawler.add_seed(entry);
        }

        let mut pages = Vec::new();
        let mut errors = Vec::new();

        while let Some(url) = crawler.next_url() {
            match self.fetch_page(&url).await {
                Ok(page) => {
                    // Add discovered links to frontier
                    for link in &page.links {
                        crawler.add_discovered(link, &url);
                    }
                    pages.push(page);
                }
                Err(e) => {
                    errors.push(CrawlError { url, error: e });
                }
            }
        }

        Ok(CrawlResult {
            pages,
            errors,
            stats: crawler.stats(),
        })
    }

    fn extract_title(&self, doc: &Html) -> Result<String, ScrapeError> {
        let selector = Selector::parse(&self.config.title_selector)
            .map_err(|_| ScrapeError::InvalidSelector)?;

        doc.select(&selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .ok_or(ScrapeError::TitleNotFound)
    }

    fn extract_links(&self, doc: &Html, base_url: &str) -> Vec<String> {
        let selector = Selector::parse("a[href]").unwrap();

        doc.select(&selector)
            .filter_map(|el| {
                el.value()
                    .attr("href")
                    .and_then(|href| resolve_url(base_url, href).ok())
            })
            .filter(|url| is_same_domain(base_url, url))
            .collect()
    }
}
```

## Testing Pattern

```rust
// tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path};

    #[tokio::test]
    async fn test_fetch_page_success() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/docs/intro"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"
                <html>
                    <head><title>Introduction</title></head>
                    <body>
                        <h1>Getting Started</h1>
                        <p>Welcome!</p>
                        <a href="/docs/next">Next</a>
                    </body>
                </html>
            "#))
            .mount(&server)
            .await;

        let config = ScrapeConfig {
            entry_points: vec![format!("{}/docs/intro", server.uri())],
            ..Default::default()
        };
        let scraper = GenericScraper::new(config).unwrap();

        let page = scraper.fetch_page(&format!("{}/docs/intro", server.uri())).await.unwrap();

        assert_eq!(page.title, "Getting Started");
        assert!(!page.links.is_empty());
    }

    #[tokio::test]
    async fn test_crawl_respects_depth_limit() {
        // Test that crawler stops at max_depth
    }

    #[tokio::test]
    async fn test_url_filter_excludes_patterns() {
        // Test URL filtering
    }
}
```

## Architectural Rules

1. Scraper **cannot import from** `commands/`, `search/`, `sync/`
2. Scraper **can import from** `config/`, `error.rs`
3. All HTTP requests **must use rate limiting**
4. All scrapers **must handle errors gracefully** (no panics)
5. Scrapers **must report progress** for UI feedback
6. Use **mock servers** for tests (never hit real URLs)
