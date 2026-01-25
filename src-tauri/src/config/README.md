# Config Directory

Configuration management and validation.

## What Belongs Here

- **Configuration types** for source YAML files
- **YAML parsing** and validation
- **Global app configuration**
- **Environment variable handling**

## What Does NOT Belong Here

- Business logic (use domain modules)
- Database operations (use `storage/`)
- File watching (use dedicated watcher module)

## Module Structure

```
config/
├── mod.rs              # Module exports
├── source.rs           # Source configuration types
├── app.rs              # Global app configuration
├── validation.rs       # Config validation logic
└── tests.rs            # Unit tests
```

## Source Configuration

```rust
// source.rs
use serde::{Deserialize, Serialize};
use crate::error::ConfigError;

/// Complete source configuration (from YAML file)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourceConfig {
    /// Display name
    pub name: String,
    /// Source type and settings
    pub source: SourceType,
    /// Version identifier (optional)
    #[serde(default)]
    pub version: Option<String>,
    /// Category for organization
    #[serde(default = "default_category")]
    pub category: String,
    /// Display settings
    #[serde(default)]
    pub display: DisplayConfig,
    /// Sync settings
    #[serde(default)]
    pub sync: SyncConfig,
}

fn default_category() -> String {
    "Uncategorized".into()
}

/// Source type variants
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SourceType {
    ReadTheDocs { url: String },
    Rustdoc { url: String },
    MdBook { url: String },
    Man {
        paths: Vec<String>,
        #[serde(default)]
        sections: Vec<u8>,
    },
    Generic {
        url: String,
        generic: GenericConfig,
    },
    Local { path: String },
}

/// Configuration for generic scraper
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GenericConfig {
    /// Entry point paths (relative to URL)
    #[serde(default = "default_entry_points")]
    pub entry_points: Vec<String>,
    /// Maximum crawl depth
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    /// CSS selector for main content
    pub content_selector: String,
    /// CSS selector for page title
    pub title_selector: String,
    /// CSS selector for navigation (optional)
    #[serde(default)]
    pub nav_selector: Option<String>,
    /// URL patterns to include (regex)
    #[serde(default)]
    pub include_patterns: Vec<String>,
    /// URL patterns to exclude (regex)
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
}

fn default_entry_points() -> Vec<String> {
    vec!["/".into()]
}

fn default_max_depth() -> u32 {
    4
}

/// Display configuration
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DisplayConfig {
    /// Icon emoji or URL
    #[serde(default)]
    pub icon: Option<String>,
    /// Accent color (hex)
    #[serde(default)]
    pub accent_color: Option<String>,
}

/// Sync configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SyncConfig {
    /// When to sync
    #[serde(default)]
    pub strategy: SyncStrategy,
    /// Cron expression for scheduled sync
    #[serde(default)]
    pub schedule: Option<String>,
    /// Pin to specific version
    #[serde(default)]
    pub pin_version: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            strategy: SyncStrategy::Manual,
            schedule: None,
            pin_version: false,
        }
    }
}

/// Sync strategy options
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncStrategy {
    #[default]
    Manual,
    OnLaunch,
    Scheduled,
    Watch,
}

impl SourceConfig {
    /// Parse from YAML string
    pub fn from_yaml(yaml: &str) -> Result<Self, ConfigError> {
        serde_yaml::from_str(yaml).map_err(ConfigError::YamlParse)
    }

    /// Parse from file path
    pub fn from_file(path: &std::path::Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::FileRead(path.to_path_buf(), e))?;
        Self::from_yaml(&content)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Name must not be empty
        if self.name.trim().is_empty() {
            return Err(ConfigError::Validation("name cannot be empty".into()));
        }

        // Validate URL for web sources
        match &self.source {
            SourceType::ReadTheDocs { url }
            | SourceType::Rustdoc { url }
            | SourceType::MdBook { url }
            | SourceType::Generic { url, .. } => {
                if !url.starts_with("https://") {
                    return Err(ConfigError::Validation(
                        "URL must use HTTPS".into()
                    ));
                }
            }
            SourceType::Local { path } => {
                if !std::path::Path::new(path).exists() {
                    return Err(ConfigError::Validation(
                        format!("Local path does not exist: {}", path)
                    ));
                }
            }
            SourceType::Man { paths, .. } => {
                if paths.is_empty() {
                    return Err(ConfigError::Validation(
                        "Man paths cannot be empty".into()
                    ));
                }
            }
        }

        // Validate generic config selectors
        if let SourceType::Generic { generic, .. } = &self.source {
            validate_css_selector(&generic.content_selector)?;
            validate_css_selector(&generic.title_selector)?;
        }

        // Validate cron schedule
        if let Some(schedule) = &self.sync.schedule {
            cron::Schedule::from_str(schedule)
                .map_err(|_| ConfigError::Validation(
                    format!("Invalid cron schedule: {}", schedule)
                ))?;
        }

        Ok(())
    }
}

fn validate_css_selector(selector: &str) -> Result<(), ConfigError> {
    scraper::Selector::parse(selector)
        .map_err(|_| ConfigError::Validation(
            format!("Invalid CSS selector: {}", selector)
        ))?;
    Ok(())
}
```

## Example YAML Files

```yaml
# ~/.tome/sources/rust-std.yaml
name: Rust Standard Library
source:
  type: rustdoc
  url: https://doc.rust-lang.org/std/
version: "1.75"
category: Language
display:
  icon: "🦀"
  accent_color: "#DEA584"
sync:
  strategy: on_launch
```

```yaml
# ~/.tome/sources/polars.yaml
name: Polars
source:
  type: generic
  url: https://docs.pola.rs/
  generic:
    entry_points: ["/"]
    max_depth: 4
    content_selector: "main.content, article"
    title_selector: "h1"
    include_patterns:
      - "^/py-polars/"
    exclude_patterns:
      - "/api/"
      - "/changelog/"
category: Python
sync:
  strategy: scheduled
  schedule: "0 0 * * 0"  # Weekly on Sunday
```

## Architectural Rules

1. Config **cannot import from any sibling directory** (pure configuration)
2. Config **can import from** `error.rs`
3. All configs must be **serializable** (for storage)
4. All configs must have **sensible defaults**
5. Validation must provide **helpful error messages**
6. Use **serde** for parsing, not manual parsing
