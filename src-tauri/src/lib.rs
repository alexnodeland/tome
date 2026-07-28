//! The Tauri application.
//!
//! Tauri **is** the native shell: it owns the process, the window, the menus,
//! and the `WKWebView`. There is no separate Swift/AppKit shell, and there is
//! no second webview — the reader will be a sandboxed `<iframe>` inside the
//! primary webview so that untrusted documentation HTML is isolated from the
//! app UI and the IPC bridge. See `docs/PRD.md` § Technical Architecture.

use tome_core::Paths;

mod spike002;

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
    match Paths::resolve().and_then(|p| p.ensure_created().map(|()| p)) {
        Ok(paths) => tracing::info!(state = %paths.state_root().display(), "library ready"),
        Err(e) => {
            tracing::error!("could not prepare the data directory: {e}");
            if let Some(hint) = e.suggestion() {
                tracing::error!("{hint}");
            }
            std::process::exit(1);
        }
    }

    let result = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            library_location,
            spike002::spike002_mode,
            spike002::spike002_page_html,
            spike002::spike002_echo,
            spike002::spike002_emit,
            spike002::spike002_report,
            spike002::spike002_done,
        ])
        .run(tauri::generate_context!());

    if let Err(e) = result {
        tracing::error!("fatal: {e}");
        std::process::exit(1);
    }
}
