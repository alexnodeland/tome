# Sync Directory

Synchronization orchestration and iCloud integration.

## What Belongs Here

- **Sync manager** orchestrating scrape -> parse -> store -> index
- **iCloud sync** for bookmarks and annotations
- **Sync scheduling** based on source configuration
- **Conflict resolution** for concurrent edits

## What Does NOT Belong Here

- HTTP fetching (delegate to `scraper/`)
- HTML parsing (delegate to `parser/`)
- Database queries (delegate to `storage/`)
- Search indexing (delegate to `search/`)

## Module Structure

```
sync/
├── mod.rs              # Module exports
├── manager.rs          # Main sync orchestrator
├── scheduler.rs        # Cron-based scheduling
├── icloud.rs           # CloudKit integration
├── conflict.rs         # Conflict resolution
└── tests.rs            # Unit tests
```

## Sync Manager

```rust
// manager.rs
use crate::{
    error::SyncError,
    scraper::{GenericScraper, ScrapeConfig, CrawlResult},
    parser::{HtmlParser, NormalizationPipeline},
    storage::{Database, FilesystemManager},
    search::SearchEngine,
};
use tokio::sync::mpsc;
use tracing::{info, warn, instrument};

/// Progress updates during sync
#[derive(Debug, Clone)]
pub enum SyncProgress {
    Started { source_id: String },
    Fetching { pages_done: u32, pages_total: Option<u32> },
    Parsing { pages_done: u32, pages_total: u32 },
    Indexing { pages_done: u32, pages_total: u32 },
    Completed { source_id: String, pages_synced: u32 },
    Failed { source_id: String, error: String },
}

/// Orchestrates the full sync pipeline
pub struct SyncManager {
    db: Database,
    fs: FilesystemManager,
    search: SearchEngine,
}

impl SyncManager {
    pub fn new(db: Database, fs: FilesystemManager, search: SearchEngine) -> Self {
        Self { db, fs, search }
    }

    /// Sync a source, optionally reporting progress
    #[instrument(skip(self, progress_tx), fields(source_id = %source_id))]
    pub async fn sync(
        &self,
        source_id: &str,
        force: bool,
        progress_tx: Option<mpsc::Sender<SyncProgress>>,
    ) -> Result<u32, SyncError> {
        // Report start
        if let Some(tx) = &progress_tx {
            let _ = tx.send(SyncProgress::Started {
                source_id: source_id.to_string(),
            }).await;
        }

        // Get source configuration
        let source = self.db.get_source(source_id).await?
            .ok_or_else(|| SyncError::SourceNotFound(source_id.to_string()))?;

        info!(source_type = %source.source_type, "Starting sync");

        // Build scraper from source config
        let scrape_config = self.build_scrape_config(&source)?;
        let scraper = GenericScraper::new(scrape_config)?;

        // Fetch pages
        let crawl_result = self.fetch_pages(&scraper, &progress_tx).await?;

        // Parse and normalize
        let pages = self.parse_pages(&source, crawl_result, &progress_tx).await?;

        // Store to filesystem and database
        self.store_pages(&source, &pages).await?;

        // Index for search
        self.index_pages(&source, &pages, &progress_tx).await?;

        // Update source metadata
        self.db.update_source(source_id, &SourceUpdate {
            page_count: Some(pages.len() as i64),
            last_synced_at: Some(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        }).await?;

        let count = pages.len() as u32;

        // Report completion
        if let Some(tx) = &progress_tx {
            let _ = tx.send(SyncProgress::Completed {
                source_id: source_id.to_string(),
                pages_synced: count,
            }).await;
        }

        info!(pages = count, "Sync completed");
        Ok(count)
    }

    async fn fetch_pages(
        &self,
        scraper: &GenericScraper,
        progress_tx: &Option<mpsc::Sender<SyncProgress>>,
    ) -> Result<CrawlResult, SyncError> {
        // TODO: Wire up progress reporting from scraper
        scraper.crawl().await.map_err(Into::into)
    }

    async fn parse_pages(
        &self,
        source: &Source,
        crawl_result: CrawlResult,
        progress_tx: &Option<mpsc::Sender<SyncProgress>>,
    ) -> Result<Vec<ParsedPage>, SyncError> {
        let base_url = source.url.as_deref().unwrap_or("");
        let pipeline = NormalizationPipeline::new(base_url);
        let total = crawl_result.pages.len() as u32;

        let mut pages = Vec::with_capacity(crawl_result.pages.len());

        for (i, scraped) in crawl_result.pages.into_iter().enumerate() {
            // Parse HTML to AST
            let ast = HtmlParser::parse(&scraped.html)?;

            // Normalize
            let normalized = pipeline.normalize(ast);

            pages.push(ParsedPage {
                path: scraped.path,
                title: scraped.title,
                ast: normalized,
                fetched_at: scraped.fetched_at,
            });

            // Report progress
            if let Some(tx) = progress_tx {
                if i % 10 == 0 {
                    let _ = tx.send(SyncProgress::Parsing {
                        pages_done: i as u32 + 1,
                        pages_total: total,
                    }).await;
                }
            }
        }

        Ok(pages)
    }

    async fn store_pages(
        &self,
        source: &Source,
        pages: &[ParsedPage],
    ) -> Result<(), SyncError> {
        for page in pages {
            // Render AST to HTML for storage
            let html = render_ast_to_html(&page.ast);

            // Store to filesystem
            self.fs.store_page(&source.id, &page.path, &html).await?;

            // Store metadata to database
            self.db.upsert_page(&PageMetadata {
                id: format!("{}:{}", source.id, page.path),
                source_id: source.id.clone(),
                path: page.path.clone(),
                title: page.title.clone(),
                content_hash: hash_content(&html),
                last_modified: page.fetched_at.to_rfc3339(),
            }).await?;
        }

        Ok(())
    }

    async fn index_pages(
        &self,
        source: &Source,
        pages: &[ParsedPage],
        progress_tx: &Option<mpsc::Sender<SyncProgress>>,
    ) -> Result<(), SyncError> {
        let mut indexer = self.search.indexer()?;
        let total = pages.len() as u32;

        for (i, page) in pages.iter().enumerate() {
            indexer.index_page(&source.id, &page.path, &page.title, &page.ast)?;

            // Report progress
            if let Some(tx) = progress_tx {
                if i % 10 == 0 {
                    let _ = tx.send(SyncProgress::Indexing {
                        pages_done: i as u32 + 1,
                        pages_total: total,
                    }).await;
                }
            }
        }

        indexer.commit()?;
        Ok(())
    }

    fn build_scrape_config(&self, source: &Source) -> Result<ScrapeConfig, SyncError> {
        // Build config based on source type
        match source.source_type.as_str() {
            "readthedocs" => Ok(ScrapeConfig::readthedocs(source.url.as_deref().unwrap())),
            "rustdoc" => Ok(ScrapeConfig::rustdoc(source.url.as_deref().unwrap())),
            "generic" => {
                // Load from stored config
                todo!()
            }
            _ => Err(SyncError::UnsupportedSourceType(source.source_type.clone())),
        }
    }
}

struct ParsedPage {
    path: String,
    title: String,
    ast: crate::parser::ast::Node,
    fetched_at: chrono::DateTime<chrono::Utc>,
}

fn render_ast_to_html(ast: &crate::parser::ast::Node) -> String {
    // TODO: Implement AST -> HTML rendering
    String::new()
}

fn hash_content(content: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}
```

## Scheduler

```rust
// scheduler.rs
use cron::Schedule;
use std::str::FromStr;
use tokio::time::{interval, Duration};
use tracing::{info, error};

/// Schedules sync operations based on source configuration
pub struct SyncScheduler {
    manager: Arc<SyncManager>,
    db: Database,
}

impl SyncScheduler {
    pub fn new(manager: Arc<SyncManager>, db: Database) -> Self {
        Self { manager, db }
    }

    /// Start the scheduler loop
    pub async fn run(&self) {
        let mut check_interval = interval(Duration::from_secs(60));

        loop {
            check_interval.tick().await;

            if let Err(e) = self.check_and_sync().await {
                error!(error = %e, "Scheduler check failed");
            }
        }
    }

    async fn check_and_sync(&self) -> Result<(), SyncError> {
        let sources = self.db.list_sources().await?;

        for source in sources {
            if self.should_sync(&source) {
                info!(source_id = %source.id, "Triggering scheduled sync");
                let manager = self.manager.clone();
                let source_id = source.id.clone();

                // Spawn sync in background
                tokio::spawn(async move {
                    if let Err(e) = manager.sync(&source_id, false, None).await {
                        error!(source_id = %source_id, error = %e, "Scheduled sync failed");
                    }
                });
            }
        }

        Ok(())
    }

    fn should_sync(&self, source: &Source) -> bool {
        match source.sync_strategy.as_str() {
            "scheduled" => {
                // Check cron schedule
                if let Some(schedule_str) = &source.sync_schedule {
                    if let Ok(schedule) = Schedule::from_str(schedule_str) {
                        // Check if next occurrence is now
                        // ...
                        return true;
                    }
                }
                false
            }
            "on_launch" => {
                // Already handled at app startup
                false
            }
            _ => false,
        }
    }
}
```

## Testing Pattern

```rust
// tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn create_test_manager() -> (SyncManager, TempDir) {
        let dir = TempDir::new().unwrap();
        let db = Database::open_in_memory().await.unwrap();
        let fs = FilesystemManager::with_path(dir.path().to_path_buf());
        let search = SearchEngine::in_memory().unwrap();

        fs.init().await.unwrap();

        (SyncManager::new(db, fs, search), dir)
    }

    #[tokio::test]
    async fn test_sync_pipeline() {
        let (manager, _dir) = create_test_manager().await;
        let server = MockServer::start().await;

        // Setup mock documentation site
        Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"
                <html>
                <head><title>Test Docs</title></head>
                <body>
                    <h1>Welcome</h1>
                    <p>Documentation content</p>
                </body>
                </html>
            "#))
            .mount(&server)
            .await;

        // Add source
        manager.db.insert_source(&Source {
            id: "test".into(),
            name: "Test".into(),
            source_type: "generic".into(),
            url: Some(server.uri()),
            ..Default::default()
        }).await.unwrap();

        // Run sync
        let (tx, mut rx) = tokio::sync::mpsc::channel(10);
        let count = manager.sync("test", false, Some(tx)).await.unwrap();

        assert!(count > 0);

        // Verify progress events received
        let mut received_completed = false;
        while let Ok(progress) = rx.try_recv() {
            if matches!(progress, SyncProgress::Completed { .. }) {
                received_completed = true;
            }
        }
        assert!(received_completed);
    }
}
```

## Architectural Rules

1. Sync **can import from** `scraper/`, `parser/`, `storage/`, `search/`
2. Sync **cannot import from** `commands/`
3. Sync operations must be **cancellable** (use cancellation tokens)
4. Sync must **report progress** for UI feedback
5. Use **channels** for progress reporting (not callbacks)
6. Handle **partial failures** gracefully (some pages fail, others succeed)
