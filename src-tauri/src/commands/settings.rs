use tauri::State;

use crate::config::AppSettings;
use crate::state::AppState;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.settings()
}

#[tauri::command]
pub fn save_settings(settings: AppSettings, state: State<'_, AppState>) -> Result<(), String> {
    state
        .save_settings(settings)
        .map_err(|error| error.to_string())
}
