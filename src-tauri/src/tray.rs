//! Ícone e menu da bandeja do sistema.

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter};

use crate::window;

/// Evento escutado por `src/hooks/useTrayEvents.ts` para abrir o modal de configurações.
pub const OPEN_SETTINGS_EVENT: &str = "jarvis://open-settings";

const MENU_OPEN: &str = "open";
const MENU_SETTINGS: &str = "settings";
const MENU_QUIT: &str = "quit";

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, MENU_OPEN, "Abrir Jarvis", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, MENU_SETTINGS, "Configurações", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &settings, &separator, &quit])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::UnknownPath)?;

    TrayIconBuilder::with_id("jarvis-tray")
        .icon(icon)
        .tooltip("Jarvis")
        .menu(&menu)
        // Esquerdo alterna a janela; o menu fica no botão direito.
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(on_tray_icon_event)
        .build(app)?;

    Ok(())
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id.as_ref() {
        MENU_OPEN => {
            let _ = window::show(app);
        }
        MENU_SETTINGS => {
            // Mostrar primeiro: abrir o modal em uma janela escondida não ajuda ninguém.
            let _ = window::show(app);
            let _ = app.emit(OPEN_SETTINGS_EVENT, ());
        }
        MENU_QUIT => app.exit(0),
        _ => {}
    }
}

fn on_tray_icon_event(tray: &TrayIcon, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        let _ = window::toggle(tray.app_handle());
    }
}
