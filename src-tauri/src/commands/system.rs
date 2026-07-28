use tauri::AppHandle;

use crate::window;

/// Controle de janela exposto à UI. Fica no Rust (e não no `@tauri-apps/api/window`)
/// para que a bandeja, a wake word e a barra de título compartilhem exatamente a
/// mesma lógica de mostrar/esconder.
#[tauri::command]
pub fn show_window(app: AppHandle) -> Result<(), String> {
    window::show(&app)
}

#[tauri::command]
pub fn hide_window(app: AppHandle) -> Result<(), String> {
    window::hide(&app)
}

#[tauri::command]
pub fn toggle_window(app: AppHandle) -> Result<(), String> {
    window::toggle(&app)
}

#[tauri::command]
pub fn minimize_window(app: AppHandle) -> Result<(), String> {
    window::minimize(&app)
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
