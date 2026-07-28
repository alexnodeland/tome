//! SPIKE-002 harness: the reader iframe IPC bridge, measured for real.
//!
//! `docs/plans/07-technical-spikes.md` § SPIKE-002 asks whether the bridge
//! between the Rust core and the reader can move a real page and sustain
//! event traffic. The reader is a sandboxed `<iframe>` inside Tauri's primary
//! webview (see `docs/PRD.md` § Technical Architecture — there is no second
//! WKWebView), so the bridge has two legs and both need numbers:
//!
//!   Rust ──invoke/event──▶ app webview ──postMessage──▶ sandboxed iframe
//!
//! Every command here is gated on the `TOME_SPIKE_002` environment variable
//! and returns an error without it, so this is dead surface in a normal
//! launch. The frontend half lives in `src/spike/spike002.ts` and
//! `public/spike002-frame.js`; results go to stdout via [`report`] so a
//! headless run can capture them. Remove the whole module when S1-13 (the
//! real reader bridge) lands — the spike write-up in `docs/spikes/` is the
//! part that outlives it.

use std::io::Write as _;

use tauri::Emitter;

/// The spike must be asked for explicitly; these commands do nothing in a
/// normally launched app.
fn gate() -> Result<(), String> {
    if std::env::var_os("TOME_SPIKE_002").is_some() {
        Ok(())
    } else {
        Err("not in spike mode: set TOME_SPIKE_002=1".into())
    }
}

/// Not gated: this is how the frontend finds out whether to run the spike.
#[tauri::command]
pub fn spike002_mode() -> bool {
    std::env::var_os("TOME_SPIKE_002").is_some()
}

/// A deterministic ~500 KB documentation-shaped page, with hostile content
/// embedded where a real crawled page would carry it:
///
/// - a `<script>` element — must not execute when injected via `innerHTML`
///   (the HTML5 parser refuses) nor under the frame CSP
/// - an inline event handler (`onerror`) — `innerHTML` does NOT neutralise
///   these; only CSP blocks them, which is exactly what the spike must show
/// - an `<img>` pointing at a live-looking network address — must be blocked
///   by the frame CSP, not fetched
///
/// 500 KB is the P95 page-size budget from docs/plans/09 § scale assumptions.
#[tauri::command]
pub fn spike002_page_html() -> Result<String, String> {
    gate()?;
    let mut html = String::with_capacity(560 * 1024);
    html.push_str("<article>\n<h1 id=\"top\">Spike fixture page</h1>\n");
    html.push_str(
        "<script>window.__probe_script_executed = true;</script>\n\
         <img src=\"http://127.0.0.1:9/pixel.png\" alt=\"probe\" \
         onerror=\"window.__probe_handler_executed = true\">\n",
    );
    let mut section = 0usize;
    while html.len() < 500 * 1024 {
        section += 1;
        html.push_str(&format!(
            "<section>\n<h2 id=\"s{section}\">Section {section}</h2>\n"
        ));
        for para in 0..6 {
            html.push_str(&format!(
                "<p>Paragraph {para} of section {section}. The quick brown fox jumps over \
                 the lazy dog while the <code>Paths</code> type resolves \
                 <code>~/Library/Application Support/Tome</code> and the annotation layer \
                 anchors by quote plus prefix and suffix, never by character offset.</p>\n"
            ));
        }
        html.push_str(&format!(
            "<pre><code>fn section_{section}() -&gt; usize {{ {section} }}</code></pre>\n</section>\n"
        ));
    }
    html.push_str("</article>\n");
    Ok(html)
}

/// Round-trip echo for measuring `invoke` latency at various payload sizes.
#[tauri::command]
pub fn spike002_echo(payload: String) -> Result<usize, String> {
    gate()?;
    Ok(payload.len())
}

/// Rust→JS push: emit `n` events as fast as the event loop takes them. The
/// frontend times first-to-last arrival; this is the "scroll command / content
/// update" direction from the spike spec.
#[tauri::command]
pub fn spike002_emit(app: tauri::AppHandle, n: u32) -> Result<(), String> {
    gate()?;
    for i in 0..n {
        app.emit("spike002-tick", i).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// One result line to stdout. The run is captured headlessly; stdout is the
/// deliverable.
#[tauri::command]
pub fn spike002_report(line: String) -> Result<(), String> {
    gate()?;
    println!("{line}");
    Ok(())
}

/// End of run. Exits the process so a scripted run terminates on its own;
/// non-zero when any check failed so the capture script can tell.
#[tauri::command]
pub fn spike002_done(failed: bool) -> Result<(), String> {
    gate()?;
    let _ = std::io::stdout().flush();
    std::process::exit(if failed { 1 } else { 0 });
}
