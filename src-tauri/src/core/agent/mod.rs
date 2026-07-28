//! PLACEHOLDER — agente de IA (v0.2).
//!
//! Aqui entra o cliente da API da Anthropic e o loop de tool use: monta o system
//! prompt (com o `assistant_name` das settings), manda o histórico, e roda o ciclo
//! `mensagem → tool_use → tool_result → mensagem` até o modelo parar.
//!
//! As tools que ele expõe ao modelo são as capacidades dos outros módulos:
//! `core::automation` (mouse, teclado, screenshot), busca na web, e leitura da
//! memória em `storage`.
//!
//! Substitui `core::chat::mock_reply`, chamado por `commands::chat::send_message`.
//! Provável divisão: `client.rs` (HTTP/streaming), `tools.rs` (definições e dispatch),
//! `prompt.rs` (system prompt e personalidade).
