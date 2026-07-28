//! Tipos de chat e o gerador de resposta MOCK da v0.1.
//!
//! `ChatMessage` / `ChatResponse` são o contrato com o frontend (`src/types/chat.ts`).
//! Quando o agente real entrar, só `mock_reply` sai de cena — os tipos ficam.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    /// Ainda não usado pela UI; reservado para o system prompt e avisos do agente.
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub role: Role,
    pub content: String,
    /// Epoch em milissegundos — o backend é a fonte única de id e timestamp.
    pub timestamp: i64,
}

impl ChatMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role,
            content: content.into(),
            timestamp: Utc::now().timestamp_millis(),
        }
    }
}

/// Envelope da resposta. Existe para acomodar `stop_reason`, `tool_calls` e uso de
/// tokens quando o agente real entrar, sem mudar a assinatura do comando.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub message: ChatMessage,
}

impl ChatResponse {
    pub fn new(message: ChatMessage) -> Self {
        Self { message }
    }
}

/// Resposta simulada da v0.1.
///
/// PONTO DE TROCA: na v0.2 isto vira uma chamada a `core::agent`, que decide entre
/// responder direto, pesquisar ou executar uma ação. A assinatura do comando não muda.
pub fn mock_reply(assistant_name: &str, user_content: &str) -> ChatMessage {
    let content = format!(
        "[mock] Aqui é o {assistant_name}. Recebi \"{user_content}\", \
         mas ainda não tenho IA ligada — isto é o esqueleto da v0.1."
    );

    ChatMessage::new(Role::Assistant, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_reply_responde_como_assistente() {
        let reply = mock_reply("Jarvis", "oi");

        assert_eq!(reply.role, Role::Assistant);
        assert!(reply.content.contains("Jarvis"));
        assert!(reply.content.contains("oi"));
        assert!(!reply.id.is_empty());
    }

    /// O frontend depende desta forma exata (`src/types/chat.ts`).
    #[test]
    fn serializa_no_formato_do_contrato() {
        let message = ChatMessage::new(Role::User, "oi");
        let json = serde_json::to_value(&message).expect("serializa");

        assert_eq!(json["role"], "user");
        assert!(json["timestamp"].is_i64());
        assert_eq!(json["content"], "oi");

        let response = serde_json::to_value(ChatResponse::new(message)).expect("serializa");
        assert!(response["message"].is_object());
    }
}
