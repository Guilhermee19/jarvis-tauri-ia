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

/**
 * Põe uma fala do Jarvis no histórico, sem ter havido pergunta.
 *
 * É a saudação de quando o app abre — a única coisa que ele diz por conta própria. Vai
 * para o backend e não só para a tela: uma mensagem empurrada apenas no frontend sumiria
 * no `getHistory` seguinte.
 */
export function announce(content: string): Promise<void> {
  return call<void>('announce', { content })
}
