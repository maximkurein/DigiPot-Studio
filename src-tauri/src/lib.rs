//! Tauri shell: exposes the digipot-core Engine through one `rpc` command.

use digipot_core::Engine;
use serde_json::Value;
use std::sync::Mutex;
use tauri::Manager;

struct AppState(Mutex<Engine>);

/// Runs on Tauri's async pool so slow hardware I/O never blocks the UI thread.
#[tauri::command]
async fn rpc(state: tauri::State<'_, AppState>, cmd: String, args: Value) -> Result<Value, String> {
    let mut engine = state.0.lock().map_err(|_| "engine poisoned".to_string())?;
    engine.handle(&cmd, args).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_text(path: String, text: String) -> Result<(), String> {
    std::fs::write(&path, text).map_err(|e| format!("{}: {}", path, e))
}

#[tauri::command]
async fn load_text(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path, e))
}

/// Opens an http(s) link in the user's browser (About screen). Only these two
/// schemes are accepted so the webview can never launch a local program.
#[tauri::command]
async fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(format!("поддерживаются только ссылки http/https: {}", url));
    }
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(&url);
        c
    };
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", "", &url]);
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(&url);
        c
    };
    cmd.spawn().map(|_| ()).map_err(|e| format!("{}: {}", url, e))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let cfg = app.path().app_config_dir().ok().map(|d| d.join("digipot-studio.json"));
            app.manage(AppState(Mutex::new(Engine::new(cfg))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![rpc, save_text, load_text, open_url])
        .run(tauri::generate_context!())
        .expect("error while running DigiPot Studio");
}
