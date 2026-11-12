//! __TITLE__ — native desktop UI on egui (no WebView, no Node).
//! The logic lives in `__CORE__`, shared with the Tauri app.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Template helpers (canvas, fmt, palette tokens) are unused until the screens
// are ported; remove this once clippy is clean without it.
#![allow(dead_code)]

mod app;
mod canvas;
mod fmt;
mod shots;
mod theme;
mod views;
mod worker;

/// Same artwork as `src-tauri/icons/icon.icns`. Without it eframe installs its
/// default egui logo as the application icon at runtime (macOS
/// `setApplicationIconImage`), which replaces the bundle icon in the Dock.
/// Path is relative to this file: crates/<app>-egui/src → repo root.
const ICON_PNG: &[u8] = include_bytes!("../../../src-tauri/icons/icon.png");

fn main() -> eframe::Result {
    let mut viewport = eframe::egui::ViewportBuilder::default();
    match eframe::icon_data::from_png_bytes(ICON_PNG) {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(e) => eprintln!("app icon not loaded: {e}"),
    }
    let options = eframe::NativeOptions {
        viewport: viewport
            .with_title("__TITLE__")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native("__TITLE__", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
