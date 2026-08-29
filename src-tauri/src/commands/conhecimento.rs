use tauri::State;

use crate::core::memory::grafo::{montar, Grafo};
use crate::core::memory::Memoria;

/// O mapa do que o Jarvis sabe: um nó por nota, arestas por link e por semelhança.
///
/// Recarrega antes de montar porque as notas são markdown numa pasta e podem ter sido
/// editadas por fora — no Obsidian, inclusive, que é de onde veio a ideia. Um grafo montado
/// sobre a cópia em memória mostraria o estado de quando o app abriu.
#[tauri::command(async)]
pub fn knowledge_graph(memoria: State<'_, Memoria>) -> Grafo {
    memoria.recarregar();
    montar(&memoria.notas())
}

/// O texto de uma nota, para o painel lateral quando alguém clica num nó.
///
/// Separado do grafo de propósito: o grafo inteiro é pedido a cada abertura da janelinha, e
/// carregar o corpo de todas as notas junto seria mandar a base inteira pelo IPC para
/// mostrar UMA. Vazio quando a nota não existe mais — a tela trata como "nada escrito
/// ainda", que é o mesmo caso visual.
#[tauri::command(async)]
pub fn note_body(id: String, memoria: State<'_, Memoria>) -> String {
    memoria.corpo_da_nota(&id)
}
