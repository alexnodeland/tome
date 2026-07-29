//! The Tauri application.
//!
//! Tauri **is** the native shell: it owns the process, the window, the menus,
//! and the `WKWebView`. There is no separate Swift/AppKit shell, and there is
//! no second webview — the reader is a sandboxed `<iframe>` inside the
//! primary webview so that untrusted documentation HTML is isolated from the
//! app UI and the IPC bridge. See `docs/PRD.md` § Technical Architecture and
//! `docs/spikes/002-reader-iframe-bridge.md`.

use tome_core::Paths;

mod reader;
mod search;

/// Where this library lives on disk. Exposed so the UI can show it and so an
/// integration test can assert the app and the CLI agree.
#[derive(serde::Serialize)]
pub struct LibraryLocation {
    bundle_id: &'static str,
    version: &'static str,
    state_root: String,
    cache_root: String,
    initialised: bool,
}

#[tauri::command]
fn library_location() -> Result<LibraryLocation, String> {
    let paths = Paths::resolve().map_err(|e| e.to_string())?;
    Ok(LibraryLocation {
        bundle_id: tome_core::BUNDLE_ID,
        version: env!("CARGO_PKG_VERSION"),
        state_root: paths.state_root().display().to_string(),
        cache_root: paths.cache_root().display().to_string(),
        initialised: paths.state_root().exists(),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,tome=info")),
        )
        .init();

    // First launch creates the directory structure. Failing here is fatal and
    // must say why: an unwritable data directory is not recoverable at runtime.
    let paths = match Paths::resolve().and_then(|p| p.ensure_created().map(|()| p)) {
        Ok(paths) => {
            tracing::info!(state = %paths.state_root().display(), "library ready");
            paths
        }
        Err(e) => {
            tracing::error!("could not prepare the data directory: {e}");
            if let Some(hint) = e.suggestion() {
                tracing::error!("{hint}");
            }
            std::process::exit(1);
        }
    };

    // The syntax set is several megabytes of inflated syntax dumps and is
    // loaded on first use. Doing it here, before the window exists, keeps the
    // cost off the first page view rather than making one page mysteriously
    // slower than the rest.
    let _ = tome_core::highlight::Highlighter::shared();

    let protocol_paths = paths.clone();
    let result = tauri::Builder::default()
        .manage(reader::ReaderState { paths })
        // Localized assets live in the cache directory, outside the bundle,
        // so the webview cannot reach them by URL without this. The handler
        // is the app's only path from page content to the filesystem and
        // validates accordingly — see `reader.rs`.
        .register_uri_scheme_protocol(reader::ASSET_SCHEME, move |_ctx, request| {
            reader::serve_asset(&protocol_paths, &request)
        })
        .invoke_handler(tauri::generate_handler![
            library_location,
            reader::list_sources,
            reader::list_pages,
            reader::read_page,
            reader::open_external,
            search::search,
            search::source_exists,
        ])
        .run(tauri::generate_context!());

    if let Err(e) = result {
        tracing::error!("fatal: {e}");
        std::process::exit(1);
    }
}
