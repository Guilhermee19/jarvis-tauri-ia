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

/// Reescreve uma nota com o que a pessoa digitou.
///
/// **O que ele aprende sozinho erra**, e a extração automática é declaradamente
/// best-effort — uma busca por "Bitcoin preço" já virou uma nota explicando o que é a
/// moeda. Sem isto, corrigir exigia achar o arquivo na pasta, o que só serve para quem
/// sabe onde ela mora.
///
/// O tipo da nota não muda (o porquê está no `Memoria::reescrever`), e a data de
/// atualização passa a ser hoje — a nota corrigida é recente, e é assim que ela deve
/// aparecer para quem procura o que está velho.
#[tauri::command(async)]
pub fn save_note(id: String, corpo: String, memoria: State<'_, Memoria>) -> Result<(), String> {
    if corpo.trim().is_empty() {
        return Err("nota vazia — para tirá-la da memória, use o apagar".to_owned());
    }

    if memoria.reescrever(&id, &corpo) {
        Ok(())
    } else {
        Err(format!("não achei a nota \"{id}\" para reescrever"))
    }
}

/// Apaga uma nota, pelo nome exato.
///
/// Diferente do "esquece X" falado, que casa por termo e pode levar várias: aqui é a nota
/// aberta na tela, e só ela. Nota errada não é para ser corrigida sempre — às vezes o
/// certo é ela não existir.
#[tauri::command(async)]
pub fn delete_note(id: String, memoria: State<'_, Memoria>) -> Result<(), String> {
    if memoria.apagar_nota(&id) {
        Ok(())
    } else {
        Err(format!("não achei a nota \"{id}\" para apagar"))
    }
}
