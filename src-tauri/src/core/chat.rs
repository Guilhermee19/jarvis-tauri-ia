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

/// Resposta simulada.
///
/// Não é mais o caminho normal — quem responde é `core::agent`. Sobrou como rede de
/// segurança para quando o intérprete está desligado (modelo vazio nas configurações),
/// para o app continuar utilizável antes de o Ollama existir.
pub fn mock_reply_text(assistant_name: &str, user_content: &str) -> String {
    format!(
        "[mock] Aqui é o {assistant_name}. Recebi \"{user_content}\", mas o intérprete \
         está desligado — configure o modelo do Ollama em Configurações."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rede de segurança precisa dizer QUEM está falando e O QUE foi recebido —
    /// sem isso o usuário com o intérprete desligado não tem pista do que houve.
    #[test]
    fn o_mock_identifica_o_assistente_e_ecoa_o_pedido() {
        let texto = mock_reply_text("Jarvis", "oi");

        assert!(texto.contains("Jarvis"));
        assert!(texto.contains("oi"));
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
