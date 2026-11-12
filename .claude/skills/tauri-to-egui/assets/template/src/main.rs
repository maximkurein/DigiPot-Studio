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

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("__TITLE__")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native("__TITLE__", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
