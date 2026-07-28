//! Núcleo de domínio do Jarvis: a lógica que NÃO depende do Tauri.
//!
//! Os módulos daqui não conhecem `#[tauri::command]` nem janelas — quem faz essa
//! ponte é `commands/`. Manter essa separação é o que vai deixar o agente, a voz e
//! a automação serem testados sem subir o app.

pub mod agent;
pub mod automation;
pub mod chat;
pub mod voice;
