//! The core data model, frozen (implementation plan S1-1).
//!
//! Everything in Stage 1 fans out behind these types: the database schema
//! (S1-2), the config parser (S1-3), the crawler (S1-4/6), the HTML→AST
//! parser (S1-7), normalization (S1-8), and the reader (S1-13) all speak
//! them. "Frozen" means additive evolution only:
//!
//! - every enum here is `#[non_exhaustive]` — downstream `match` carries a
//!   wildcard arm, and a new node kind or source type is a minor change;
//! - the serde shape is part of the freeze. `tests/model_serde_shape.rs`
//!   asserts exact JSON; if a change breaks that test, it breaks stored data
//!   and IPC, and needs a migration, not a shrug;
//! - renaming or repurposing an existing field is off the table.
//!
//! Two deliberate divergences from the PRD's data-model sketch, both with the
//! same root cause (the sketch predates ADR-0001's sync design):
//!
//! 1. **[`SourceId`] is a validated slug, not a `Uuid`.** A source's identity
//!    is its config file name — `sources/<id>.yaml` — which is also its
//!    directory name under the cache root, what the CLI takes (`tome pull
//!    python`), and what the registry ships. Adding the same source on two
//!    devices must produce the *same* identity, or bookmark sync (per-device
//!    op logs, ADR-0001) can never merge records for it; a per-device UUID
//!    would make every source unique to the machine that added it.
//! 2. **[`Page`] has no surrogate id.** Its identity is `(source, path)` —
//!    the PRD's own `Bookmark` and `Annotation` types already reference pages
//!    by `page_path`, and the S1-2 schema enforces `UNIQUE(source_id, path)`.
//!    A row id is the database's private business.

mod docset;
mod node;
mod page;
mod source;

pub use docset::{DocPage, DocSet, TocEntry};
pub use node::{Definition, ListItem, Node, TableCell, TableRow};
pub use page::{ContentHash, Page, PagePath};
pub use source::{
    Attribution, Icon, Schedule, Source, SourceId, SourceType, SyncConfig, SyncStrategy,
};
