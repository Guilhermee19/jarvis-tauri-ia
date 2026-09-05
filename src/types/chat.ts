/**
 * Contrato de chat entre o frontend e o Rust.
 *
 * Espelha 1:1 os structs de `src-tauri/src/core/chat.rs`. Se um lado mudar, o outro
 * precisa mudar junto — é o contrato que vai continuar valendo quando o mock for
 * substituído pelo agente Claude em `core/agent`.
 */

export type ChatRole = 'user' | 'assistant' | 'system'

export interface ChatMessage {
  id: string
  role: ChatRole
  content: string
  /** Epoch em milissegundos. Gerado no backend para o histórico ter uma fonte única. */
  timestamp: number
}

/**
 * Envelope da resposta. Hoje carrega só a mensagem, mas existe justamente para
 * caber `stopReason`, `toolCalls` e uso de tokens quando o agente real entrar,
 * sem quebrar a assinatura do comando.
 */
export interface ChatResponse {
  message: ChatMessage
}

/**
 * O veredito de uma resposta. Espelha `Veredito` em
 * `src-tauri/src/core/memory/mod.rs` (serde em snake_case).
 */
export type Veredito = 'acertou' | 'passou_perto' | 'errou'

/**
 * Que tipo de erro foi.
 *
 * A distinção não é taxonomia: `fato` vira nota sobre AQUELE assunto, que volta quando o
 * assunto voltar; `jeito` não tem assunto ao qual se prender e vira regra fixa no prompt.
 */
export type ErroDaResposta = 'fato' | 'jeito'
