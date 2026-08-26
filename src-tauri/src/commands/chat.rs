use tauri::State;

use crate::core::agent::{self, AgentError};
use crate::core::chat::{ChatMessage, ChatResponse, Role};
use crate::core::services::Services;
use crate::state::AppState;

/// Recebe a mensagem do usuário, deixa o agente entender e agir, e devolve a resposta.
///
/// Uma jogada pode empurrar DUAS mensagens no histórico: o log do gatilho e a
/// resposta. A assinatura não muda por causa disso — o frontend recarrega o histórico
/// depois de enviar, que é o que mantém o espelho fiel sem inventar um segundo canal.
#[tauri::command]
pub async fn send_message(
    content: String,
    state: State<'_, AppState>,
    services: State<'_, Services>,
) -> Result<ChatResponse, String> {
    let content = content.trim().to_owned();
    if content.is_empty() {
        return Err("mensagem vazia".to_owned());
    }

    state.push_message(ChatMessage::new(Role::User, content.clone()));

    let settings = state.settings();
    let http = state.http();

    // Sobe o Ollama se ninguém atender. O resultado é ignorado de propósito: se não
    // subir, quem dá a mensagem boa (com o link e o comando do `ollama pull`) é o
    // `AgentError::Offline`, logo abaixo — reportar aqui seria pior e em dobro.
    if !settings.ollama_model.trim().is_empty() {
        let _ = services.ensure_ollama(&http, &settings.ollama_url).await;
    }

    let outcome = agent::handle(&http, &settings, &content)
        .await
        .map_err(stringify)?;

    // Ordem importa: o log entra ANTES da resposta, então a conversa se lê como
    // usuário → o que ele entendeu e fez → o que ele respondeu.
    if let Some(trace) = outcome.trace {
        state.push_message(ChatMessage::new(Role::System, trace));
    }

    let reply = ChatMessage::new(Role::Assistant, outcome.reply);
    state.push_message(reply.clone());

    Ok(ChatResponse::new(reply))
}

fn stringify(error: AgentError) -> String {
    error.to_string()
}

/// A UI chama isto ao montar: como o histórico é do backend, a janela pode ser
/// escondida e reaberta sem perder a conversa.
#[tauri::command]
pub fn get_history(state: State<'_, AppState>) -> Vec<ChatMessage> {
    state.history()
}

#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) {
    state.clear_history();
}
