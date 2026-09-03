import type { ChatMessage, ChatResponse } from '@/types'
import { call } from './client'

/** Wrappers de `src-tauri/src/commands/chat.rs`. */

/**
 * O histórico mora no backend, então enviar uma mensagem devolve só a resposta —
 * quando o agente real entrar, essa mesma chamada é que vai rodar o loop de tool use.
 *
 * **Só volta quando ele terminou de FALAR**, e não quando o texto ficou pronto: a resposta
 * sai em fluxo, e o Rust espera a última frase calar. É esse retorno que o modo conversa
 * usa para saber quando reabrir o microfone.
 *
 * `turno` é o crachá das frases: cada uma volta por evento carimbada com ele, e é assim
 * que uma resposta interrompida não escreve dentro da bolha da resposta seguinte. Quem o
 * gera é o chamador, porque só ele sabe qual turno está na tela.
 */
export function sendMessage(content: string, turno: string): Promise<ChatResponse> {
  return call<ChatResponse>('send_message', { content, turno })
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
