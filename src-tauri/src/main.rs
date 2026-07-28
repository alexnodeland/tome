// Release builds must not spawn a console window on Windows. Tome is macOS-only
// today, but the attribute costs nothing and removes a future footgun.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tome_app_lib::run()
}
