use tauri::State;

use crate::core::chat::{mock_reply, ChatMessage, ChatResponse, Role};
use crate::state::AppState;

/// Recebe a mensagem do usuário e devolve a resposta do assistente.
///
/// PONTO DE TROCA: hoje chama `core::chat::mock_reply`; na v0.2 passa a chamar
/// `core::agent`, que roda o loop de tool use. Assinatura e tipos não mudam — por
/// isso já é `async`.
#[tauri::command]
pub async fn send_message(
    content: String,
    state: State<'_, AppState>,
) -> Result<ChatResponse, String> {
    let content = content.trim().to_owned();
    if content.is_empty() {
        return Err("mensagem vazia".to_owned());
    }

    state.push_message(ChatMessage::new(Role::User, content.clone()));

    let reply = mock_reply(&state.settings().assistant_name, &content);
    state.push_message(reply.clone());

    Ok(ChatResponse::new(reply))
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
