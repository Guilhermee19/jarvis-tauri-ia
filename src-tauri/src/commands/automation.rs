use tauri::State;

use crate::core::automation::{
    capture_screen, list_monitors as monitors, AutomationError, AutomationState, CapturedImage,
    MonitorInfo,
};

/// `(async)` em tudo que fala com hardware: comando síncrono roda na thread
/// principal do Tauri, e abrir uma câmera ou capturar a tela leva tempo suficiente
/// para a janela congelar no meio do preview.
#[tauri::command(async)]
pub fn open_webcam(automation: State<'_, AutomationState>) -> Result<(), String> {
    automation.open_webcam().map_err(stringify)
}

#[tauri::command]
pub fn close_webcam(automation: State<'_, AutomationState>) {
    automation.close_webcam();
}

#[tauri::command]
pub fn is_webcam_open(automation: State<'_, AutomationState>) -> bool {
    automation.is_webcam_open()
}

/// Serve tanto ao preview (chamado em laço) quanto ao botão de capturar frame — é a
/// mesma imagem, e quem decide o que fazer com ela é quem chamou.
#[tauri::command(async)]
pub fn capture_webcam_frame(
    automation: State<'_, AutomationState>,
) -> Result<CapturedImage, String> {
    automation.capture_webcam_frame().map_err(stringify)
}

#[tauri::command(async)]
pub fn list_monitors() -> Result<Vec<MonitorInfo>, String> {
    monitors().map_err(stringify)
}

#[tauri::command(async)]
pub fn capture_screenshot(monitor_id: Option<u32>) -> Result<CapturedImage, String> {
    capture_screen(monitor_id).map_err(stringify)
}

fn stringify(error: AutomationError) -> String {
    error.to_string()
}
