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

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("DigiPot Studio — MCP4161")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native("DigiPot Studio", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
