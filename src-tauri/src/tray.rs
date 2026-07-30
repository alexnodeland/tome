//! The menu bar item and the global shortcut (S4-6, P5-008/009).
//!
//! **SPIKE-001 asked whether native menu bar integration needs a Swift AppKit
//! shell. It does not** — see `docs/spikes/001-menu-bar.md`. Tauri's
//! `tray-icon` feature is `NSStatusItem` with a Rust surface, and
//! `tauri-plugin-global-shortcut` is `RegisterEventHotKey`. Neither needs
//! Objective-C, a plugin bridge, or a second process, and both are in this
//! file. The spike's fallback ("pure Tauri menu bar, limited features") turned
//! out to be the whole answer.
//!
//! Three decisions worth keeping:
//!
//! 1. **The global shortcut is off by default.** PRD Appendix C says so, and
//!    it is right: a system-wide hotkey claimed at first launch is a hotkey
//!    taken from whatever the user had bound to it, silently.
//! 2. **Registration success does not mean the shortcut works.** Measured
//!    during SPIKE-001: registering `CmdOrCtrl+Space` — which Spotlight owns —
//!    **succeeds**, and the handler then never fires, because the system
//!    consumes the keystroke first. `RegisterEventHotKey` only refuses a
//!    combination held by another *application* hotkey, not one reserved by
//!    macOS itself, and no API lists either. So registration failure is
//!    reported, and it is necessary but not sufficient: the frontend also
//!    refuses combinations known to be reserved, because a silently dead
//!    shortcut is the worse failure of the two.
//! 3. **Activating shows the window and opens search.** Not just "raise the
//!    window": someone who pressed a global shortcut is looking for something,
//!    and the extra ⌘K is a keystroke they should not have to make.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

// The default combination (PRD Appendix C: ⌘⇧D) is NOT defined here. It is a
// preference, it lives with every other preference in the frontend's store,
// and a copy in Rust would be a second default that drifts from the first.
// Rust is told what to register; it does not decide.

/// What the frontend should do when the app is activated from outside.
///
/// A string rather than a bool, because "raise the window" and "raise the
/// window and open search" are different intents and the tray menu needs
/// both.
#[derive(serde::Serialize, Clone)]
struct Activate {
    /// `search`, `catalogue`, or `window`.
    intent: &'static str,
}

/// Bring the app forward and tell the UI why.
fn activate<R: Runtime>(app: &AppHandle<R>, intent: &'static str) {
    if let Some(window) = app.get_webview_window("main") {
        // `show` before `set_focus`: a window hidden by the close button (or
        // by accessory activation policy) cannot take focus while hidden, and
        // the focus call silently does nothing.
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    // Emitted even when the window is missing, so that a UI listening on a
    // future second window still hears it.
    let _ = app.emit("activate", Activate { intent });
}

/// Build the menu bar item.
pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Tome", true, None::<&str>)?;
    let search = MenuItem::with_id(app, "search", "Search…", true, None::<&str>)?;
    let add = MenuItem::with_id(app, "add", "Add a source…", true, None::<&str>)?;
    let quit = PredefinedMenuItem::quit(app, Some("Quit Tome"))?;
    // Deliberately short. P5-008 also asked for recent searches and bookmarks
    // in this menu: search history lives in the frontend's localStorage and is
    // not readable from here, and there are no bookmarks yet. Both belong in
    // the change that makes them reachable, not in a menu that shows an empty
    // section.
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &search,
            &add,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;

    TrayIconBuilder::with_id("tome")
        .icon(icon)
        // A template image is recoloured by macOS for light, dark, and
        // highlighted states. Without this the icon is drawn as-is and is
        // invisible in one of them.
        .icon_as_template(true)
        .tooltip("Tome")
        .menu(&menu)
        // Left-click opens the app rather than the menu; right-click opens the
        // menu, which is what `on_tray_icon_event` below leaves to the system.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => activate(app, "window"),
            "search" => activate(app, "search"),
            "add" => activate(app, "catalogue"),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                // `Up`, not `Down`: acting on the press makes a click-and-drag
                // to reposition the item in the menu bar also open the app.
                activate(tray.app_handle(), "search");
            }
        })
        .build(app)?;

    // Logged on success, not only on failure. SPIKE-001's question is whether
    // this works without a Swift shell, and "no warning appeared" is weaker
    // evidence than a line that says it did.
    tracing::info!("menu bar item created");
    Ok(())
}

/// Register the global shortcut, replacing whatever was registered before.
///
/// `None` unregisters and registers nothing, which is the default state.
/// Returns the error text on failure rather than an `Err`, because the caller
/// is a Tauri command whose whole job is to hand that string to the UI.
pub fn set_shortcut<R: Runtime>(
    app: &AppHandle<R>,
    accelerator: Option<&str>,
) -> Result<(), String> {
    let manager = app.global_shortcut();
    // Unregister everything first. Registering a second combination without
    // this leaves the first one live, so a user who changed the shortcut would
    // find both working and no way to discover why.
    manager
        .unregister_all()
        .map_err(|e| format!("could not release the previous shortcut: {e}"))?;

    let Some(accelerator) = accelerator else {
        return Ok(());
    };

    let shortcut: Shortcut = accelerator
        .parse()
        .map_err(|_| format!("`{accelerator}` is not a shortcut Tome understands."))?;

    let handle = app.clone();
    manager
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            // Key *press* only. Without this filter the handler runs twice per
            // keystroke and the window is raised, then raised again.
            if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                activate(&handle, "search");
            }
        })
        .map_err(|e| {
            // This catches only half the conflicts — another application's
            // hotkey. A combination macOS itself reserves registers cleanly
            // and never fires; see the module docs and `reservedShortcut` in
            // the frontend, which is the other half.
            format!(
                "`{accelerator}` could not be registered — another application is probably \
                 already using it. Try a different combination. ({e})"
            )
        })?;

    tracing::info!(accelerator, "global shortcut registered");
    Ok(())
}
