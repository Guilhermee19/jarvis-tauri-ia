use crate::core::casa::{descobrir, CasaError, Varredura};

/// Escuta a rede à procura de aparelhos Tuya (Positivo, EKAZA e companhia).
///
/// `(async)` porque **bloqueia por segundos**: não é uma consulta, é uma janela de
/// escuta — os aparelhos se anunciam sozinhos de tempos em tempos e não há como pedir
/// que falem antes da hora. Comando síncrono aqui congelaria a janela inteira.
#[tauri::command(async)]
pub fn discover_devices() -> Result<Varredura, String> {
    descobrir().map_err(|erro: CasaError| erro.to_string())
}
