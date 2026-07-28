//! Montagem do app: estado, bandeja, eventos de janela e registro dos comandos.
//!
//! Serviços de background (wake word, escuta contínua) entram no `setup`, como
//! tasks que emitem eventos para a UI.

mod commands;
mod config;
mod core;
mod state;
mod storage;
mod tray;
mod window;

use tauri::Manager;

use crate::core::automation::AutomationState;
use crate::core::voice::VoiceState;
use crate::state::AppState;
use crate::storage::json_store::JsonSettingsStore;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            app.manage(AppState::new(Box::new(JsonSettingsStore::new(&config_dir))));
            // Estados separados por capacidade: o `AppState` cuida de conversa e
            // configurações, e cada sensor é dono só do que ele abre.
            app.manage(VoiceState::new());
            app.manage(AutomationState::new());

            tray::build(app.handle())?;
            Ok(())
        })
        .on_window_event(window::handle_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::chat::send_message,
            commands::chat::get_history,
            commands::chat::clear_history,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::system::show_window,
            commands::system::hide_window,
            commands::system::toggle_window,
            commands::system::minimize_window,
            commands::system::quit_app,
            commands::voice::start_recording,
            commands::voice::stop_recording,
            commands::voice::is_recording,
            commands::voice::list_voices,
            commands::voice::speak_text,
            commands::automation::open_webcam,
            commands::automation::close_webcam,
            commands::automation::is_webcam_open,
            commands::automation::capture_webcam_frame,
            commands::automation::list_monitors,
            commands::automation::capture_screenshot,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Jarvis");
}
