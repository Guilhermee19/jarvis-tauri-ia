//! Núcleo de domínio do Jarvis: a lógica que NÃO depende do Tauri.
//!
//! Os módulos daqui não conhecem `#[tauri::command]` nem janelas — quem faz essa
//! ponte é `commands/`. Manter essa separação é o que vai deixar o agente, a voz e
//! a automação serem testados sem subir o app.

pub mod agent;
pub mod automation;
pub mod chat;
pub mod services;
pub mod system;
pub mod voice;

use std::sync::{Mutex, MutexGuard};

/// Um mutex envenenado não deve derrubar o app: os dados protegidos aqui são
/// simples (uma gravação, uma câmera, um histórico) e continuam consistentes,
/// então seguimos com o valor de dentro.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
