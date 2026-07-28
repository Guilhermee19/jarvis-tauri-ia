import type { ChatMessage, ChatResponse } from '@/types'
import { call } from './client'

/** Wrappers de `src-tauri/src/commands/chat.rs`. */

/**
 * O histórico mora no backend, então enviar uma mensagem devolve só a resposta —
 * quando o agente real entrar, essa mesma chamada é que vai rodar o loop de tool use.
 */
export function sendMessage(content: string): Promise<ChatResponse> {
  return call<ChatResponse>('send_message', { content })
}

export function getHistory(): Promise<ChatMessage[]> {
  return call<ChatMessage[]>('get_history')
}

export function clearHistory(): Promise<void> {
  return call<void>('clear_history')
}
