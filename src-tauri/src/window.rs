//! Ciclo de vida da janela principal.
//!
//! Um lugar só para mostrar/esconder, usado pela bandeja, pela barra de título e
//! (futuramente) pela wake word, que precisa trazer a janela para frente ao ouvir
//! o gatilho.

use tauri::{AppHandle, Manager, WebviewWindow, Window, WindowEvent};

pub const MAIN_WINDOW_LABEL: &str = "main";

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| format!("janela \"{MAIN_WINDOW_LABEL}\" não encontrada"))
}

fn to_message(error: tauri::Error) -> String {
    error.to_string()
}

pub fn show(app: &AppHandle) -> Result<(), String> {
    let window = main_window(app)?;
    window.unminimize().map_err(to_message)?;
    window.show().map_err(to_message)?;
    window.set_focus().map_err(to_message)?;
    Ok(())
}

pub fn hide(app: &AppHandle) -> Result<(), String> {
    main_window(app)?.hide().map_err(to_message)
}

pub fn minimize(app: &AppHandle) -> Result<(), String> {
    main_window(app)?.minimize().map_err(to_message)
}

pub fn toggle(app: &AppHandle) -> Result<(), String> {
    let window = main_window(app)?;

    // Minimizada ainda conta como "visível" no Windows, então esconder nesse caso
    // faria o clique na bandeja parecer que não fez nada.
    let is_showing =
        window.is_visible().map_err(to_message)? && !window.is_minimized().map_err(to_message)?;

    if is_showing {
        hide(app)
    } else {
        show(app)
    }
}

/// Fechar no X esconde para a bandeja em vez de encerrar — o Jarvis é feito para
/// ficar sempre rodando em background. Sair de verdade só pelo menu da bandeja.
pub fn handle_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        if window.label() == MAIN_WINDOW_LABEL {
            api.prevent_close();
            let _ = window.hide();
        }
    }
}
