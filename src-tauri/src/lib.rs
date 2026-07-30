//! The Tauri application.
//!
//! Tauri **is** the native shell: it owns the process, the window, the menus,
//! and the `WKWebView`. There is no separate Swift/AppKit shell, and there is
//! no second webview — the reader is a sandboxed `<iframe>` inside the
//! primary webview so that untrusted documentation HTML is isolated from the
//! app UI and the IPC bridge. See `docs/PRD.md` § Technical Architecture and
//! `docs/spikes/002-reader-iframe-bridge.md`.

use tome_core::Paths;

mod onboarding;
mod reader;
mod search;
mod tray;

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
    // Debug rather than info: this is per-command observability
    // (`docs/plans/13`), off in a user's log and on under
    // `RUST_LOG=tome=debug`. `scripts/measure-startup.sh` uses this specific
    // line as its "the UI is up and talking" marker, because it is the first
    // call the frontend makes.
    tracing::debug!("library_location");
    let paths = Paths::resolve().map_err(|e| e.to_string())?;
    Ok(LibraryLocation {
        bundle_id: tome_core::BUNDLE_ID,
        version: env!("CARGO_PKG_VERSION"),
        state_root: paths.state_root().display().to_string(),
        cache_root: paths.cache_root().display().to_string(),
        initialised: paths.state_root().exists(),
    })
}

/// Register (or clear) the system-wide shortcut.
///
/// Called by the frontend at startup with whatever it has stored, and again
/// whenever the preference changes — the accelerator lives in the same
/// `localStorage` as every other preference, so Rust is told rather than
/// asked. Returns the failure text: on macOS a refused registration is the
/// only conflict detection there is.
#[tauri::command]
fn set_global_shortcut(app: tauri::AppHandle, accelerator: Option<String>) -> Result<(), String> {
    tray::set_shortcut(&app, accelerator.as_deref())
}

/// Show or hide the Dock icon (P5-008's "hide from dock").
///
/// `Accessory` is what makes an application menu-bar-only. Hiding the Dock
/// icon while the window is closed would leave no way back in, which is why
/// the menu bar item is created unconditionally and before this can be called.
#[tauri::command]
fn set_dock_visible(app: tauri::AppHandle, visible: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let policy = if visible {
            tauri::ActivationPolicy::Regular
        } else {
            tauri::ActivationPolicy::Accessory
        };
        app.set_activation_policy(policy)
            .map_err(|e| format!("could not change the Dock setting: {e}"))?;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, visible);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // stderr and a rotated file under the library's `logs/` (S4-3). The app
    // matters more than the CLI here: a GUI has no terminal to print to, so
    // without the file half every diagnostic it emits is written to a stream
    // nobody is reading. The file is created on the first event, not now — a
    // launch that fails before it logs anything leaves no directory behind.
    let filter = || {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,tome=info"))
    };
    match Paths::resolve() {
        Ok(paths) => tracing_subscriber::fmt()
            .with_writer(tome_core::logging::to_stderr_and_file(&paths))
            .with_ansi(false)
            .with_env_filter(filter())
            .init(),
        Err(_) => tracing_subscriber::fmt().with_env_filter(filter()).init(),
    }

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

    // The syntax set, warmed before the window exists.
    //
    // The comment here used to say this "keeps the cost off the first page
    // view". **Measured at S4-2: the cost is 0 ms.** syntect's bundled
    // defaults are lump data that is not parsed at load, so warming it buys
    // nothing measurable. It is kept because it also costs nothing, and
    // because it makes the first page view's cost unambiguous — but the
    // justification is now a measurement rather than an assumption.
    let _ = tome_core::highlight::Highlighter::shared();

    let protocol_paths = paths.clone();
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
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
            onboarding::registry_catalogue,
            onboarding::install_registry_source,
            set_global_shortcut,
            set_dock_visible,
        ])
        .setup(|app| {
            // The menu bar item. A failure here is logged and not fatal: an
            // app that refuses to launch because it could not draw a status
            // item would be unusable for a feature that is an accessory.
            if let Err(e) = tray::install(app.handle()) {
                tracing::warn!("the menu bar item could not be created: {e}");
            }
            Ok(())
        })
        .run(tauri::generate_context!());

    if let Err(e) = result {
        tracing::error!("fatal: {e}");
        std::process::exit(1);
    }
}
