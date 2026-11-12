//! DigiPot Studio — native desktop UI on egui (no WebView, no Node).
//! The engineering model, simulator and hardware links are `digipot-core`,
//! shared with the Tauri app and `digipot-server`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod canvas;
mod content;
mod fmt;
mod shots;
mod theme;
mod views;
mod widgets;
mod worker;

/// Same artwork as `src-tauri/icons/icon.icns`. Without it eframe installs its
/// default egui logo as the application icon at runtime (macOS
/// `setApplicationIconImage`), which replaces the bundle icon in the Dock.
const ICON_PNG: &[u8] = include_bytes!("../../../src-tauri/icons/icon.png");

fn main() -> eframe::Result {
    let mut viewport = eframe::egui::ViewportBuilder::default();
    match eframe::icon_data::from_png_bytes(ICON_PNG) {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(e) => eprintln!("иконка приложения не загружена: {e}"),
    }
    let options = eframe::NativeOptions {
        viewport: viewport
            .with_title("DigiPot Studio — MCP4161")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native("DigiPot Studio", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
