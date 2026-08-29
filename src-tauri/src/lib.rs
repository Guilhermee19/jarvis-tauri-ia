//! Montagem do app: estado, bandeja, eventos de janela e registro dos comandos.
//!
//! Serviços de background (wake word, escuta contínua) entram no `setup`, como
//! tasks que emitem eventos para a UI.

mod commands;
mod config;
mod core;
mod navegador;
mod state;
mod storage;
mod tray;
mod window;

use std::path::{Path, PathBuf};

use tauri::Manager;

use crate::config::AppSettings;
use crate::core::automation::AutomationState;
use crate::core::casa::chaveiro::Chaveiro;
use crate::core::memory::Memoria;
use crate::core::services::Services;
use crate::core::voice::VoiceState;
use crate::state::AppState;
use crate::storage::json_store::JsonSettingsStore;

/// Onde fica a pasta de memória — a de markdown que dá para abrir no Obsidian.
///
/// Em desenvolvimento ela mora NO PROJETO, ao lado do código: é assim que dá para
/// versionar o que ele aprendeu e ver a memória crescer no diff. Num app instalado
/// isso não serve (ninguém escreve em Program Files), então cai no diretório de dados
/// do usuário. As configurações vencem os dois.
fn pasta_da_memoria(settings: &AppSettings, app: &tauri::AppHandle) -> PathBuf {
    let escolhida = settings.memoria_path.trim();
    if !escolhida.is_empty() {
        return PathBuf::from(escolhida);
    }

    if cfg!(debug_assertions) {
        if let Some(projeto) = Path::new(env!("CARGO_MANIFEST_DIR")).parent() {
            return projeto.join("memoria");
        }
    }

    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("memoria")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let state = AppState::new(Box::new(JsonSettingsStore::new(&config_dir)));

            let pasta = pasta_da_memoria(&state.settings(), app.handle());
            println!("[jarvis] memória em {}", pasta.display());
            app.manage(Memoria::new(&pasta));

            app.manage(state);
            // Estados separados por capacidade: o `AppState` cuida das configurações,
            // a `Memoria` do que ele lembra, e cada sensor é dono só do que ele abre.
            app.manage(VoiceState::new());
            app.manage(AutomationState::new());
            // Dono dos processos externos (whisper-server, ollama). Nada sobe aqui:
            // a subida é preguiçosa, na primeira vez que a voz é usada.
            app.manage(Services::new());
            // Os contadores de desempenho, que só abrem na primeira vez que o painel
            // perguntar — e ficam abertos depois, porque uso de CPU é uma taxa entre
            // duas leituras.
            app.manage(crate::core::system::DesempenhoState::new());
            // As abas do navegador interno. Vazio até alguém pedir um site.
            app.manage(crate::navegador::Navegador::new());
            // As chaves dos aparelhos da casa. Fica ao lado do `AppState` e não dentro
            // dele porque é DADO buscado, não configuração escolhida — e porque o botão
            // de importar reescreve o arquivo inteiro, o que ninguém quer que aconteça
            // com o `settings.json`.
            app.manage(Chaveiro::new(&config_dir));

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
            commands::system::toggle_maximize_window,
            commands::system::is_window_maximized,
            commands::system::now_playing,
            commands::system::press_media_key,
            commands::system::identify_track,
            commands::system::quit_app,
            commands::system::performance_metrics,
            commands::navegador::browser_open,
            commands::navegador::browser_search,
            commands::navegador::browser_state,
            commands::navegador::browser_select,
            commands::navegador::browser_close,
            commands::navegador::browser_navigate,
            commands::navegador::browser_history,
            commands::navegador::browser_bounds,
            commands::navegador::browser_external,
            commands::voice::start_recording,
            commands::voice::stop_recording,
            commands::voice::is_recording,
            commands::voice::list_voices,
            commands::voice::upload_voice_reference,
            commands::voice::list_input_devices,
            commands::voice::speak_text,
            commands::voice::stop_speaking,
            commands::voice::transcribe,
            commands::casa::discover_devices,
            commands::casa::known_devices,
            commands::casa::import_tuya_devices,
            commands::casa::set_device_power,
            commands::casa::device_state,
            commands::casa::set_light,
            commands::casa::set_device_dp,
            commands::casa::sensor_states,
            commands::casa::set_device_hidden,
            commands::casa::ir_keys,
            commands::casa::send_ir_key,
            commands::automation::open_webcam,
            commands::automation::close_webcam,
            commands::automation::is_webcam_open,
            commands::automation::capture_webcam_frame,
            commands::automation::list_webcam_resolutions,
            commands::automation::list_monitors,
            commands::automation::capture_screenshot,
        ])
        .build(tauri::generate_context!())
        .expect("erro ao iniciar o Jarvis");

    // `RunEvent::Exit` em vez de mexer no `quit_app` e no menu da bandeja: são duas
    // portas de saída hoje, e qualquer terceira que apareça amanhã passa por aqui.
    app.run(|handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            handle.state::<Services>().shutdown();
        }
    });
}
