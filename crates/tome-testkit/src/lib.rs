//! Test infrastructure for Tome.
//!
//! Two things live here, both from Stage 0 of
//! `docs/plans/18-implementation-plan.md`, and both prerequisites for any
//! ingestion code:
//!
//! - [`server`] — S0-6. An HTTP server that serves committed documentation
//!   fixtures with no network. Every scraper test needs one, and the offline
//!   guarantee is only assertable if the server can be **shut down mid-test**.
//! - [`golden`] — S0-7. A snapshot harness for output that is *judged* rather
//!   than asserted: normalization, rendering, snippet generation. It turns
//!   "does this HTML look right?" into a reviewable diff.
//!
//! This crate is a **dev-dependency only**. Nothing it contains is compiled
//! into `tome`, `Tome.app`, or the MCP server. `publish = false` and the
//! absence of any `[dependencies]` entry in the shipping crates are what keep
//! that true; if it ever appears under `[dependencies]`, that is a bug.
//!
//! # Why the fixture server is hand-written
//!
//! It is ~400 lines over `std::net` instead of ten lines over axum. Two
//! reasons, in order of weight:
//!
//! 1. **A fixture server has to misbehave on purpose.** Scripted 429s with
//!    `Retry-After`, redirect chains, mid-body disconnects, and a shutdown that
//!    makes the port refuse connections are the interesting cases. A correct
//!    HTTP stack is built to resist exactly those.
//! 2. It is a dev-dependency of every crate, so its own dependencies are
//!    compiled on every `cargo test`.
//!
//! The cost is real and worth stating: this is a deliberately partial HTTP/1.1
//! implementation. No keep-alive (every response closes the connection), no
//! chunked encoding, no compression, no HTTP/2. Those are documented on
//! [`server::FixtureServer`] rather than discovered.

pub mod golden;
pub mod server;

pub use golden::{Golden, Outcome, Report};
pub use server::{FixtureServer, Request, Scripted};
