use tauri::State;

use crate::core::automation::{
    capture_screen, list_monitors as monitors, list_webcam_resolutions as resolutions,
    AutomationError, AutomationState, CapturedImage, MonitorInfo, WebcamResolution,
};
use crate::state::AppState;

/// `(async)` em tudo que fala com hardware: comando síncrono roda na thread
/// principal do Tauri, e abrir uma câmera ou capturar a tela leva tempo suficiente
/// para a janela congelar no meio do preview.
#[tauri::command(async)]
pub fn open_webcam(
    automation: State<'_, AutomationState>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    automation
        .open_webcam(state.settings().webcam_target())
        .map_err(stringify)
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
/// `maxWidth` é o teto da imagem DEVOLVIDA, em pixels — a captura segue na resolução
/// configurada. O laço da prévia manda a largura da janela; quem quer o quadro
/// inteiro (o botão de capturar, o agente) omite.
#[tauri::command(async)]
pub fn capture_webcam_frame(
    automation: State<'_, AutomationState>,
    state: State<'_, AppState>,
    max_width: Option<u32>,
) -> Result<CapturedImage, String> {
    automation
        .capture_webcam_frame(
            state.settings().webcam_target(),
            max_width.filter(|t| *t > 0),
        )
        .map_err(stringify)
}

/// O que a câmera declara suportar, para as configurações oferecerem o que EXISTE.
///
/// Uma lista fixa (720p, 1080p) mentiria em metade das webcams: a escolha do usuário
/// seria silenciosamente trocada pelo formato mais próximo, e não haveria como saber
/// disso pela tela.
#[tauri::command(async)]
pub fn list_webcam_resolutions() -> Result<Vec<WebcamResolution>, String> {
    resolutions().map_err(stringify)
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
