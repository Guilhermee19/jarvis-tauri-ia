import type { Grafo } from '@/types'
import { call } from './client'

/** Wrappers de `src-tauri/src/commands/conhecimento.rs`. */

/**
 * O mapa do que o Jarvis sabe.
 *
 * Ele recarrega as notas do disco antes de montar, então editar um `.md` por fora (no
 * Obsidian, por exemplo) aparece aqui no próximo clique em atualizar.
 */
export function knowledgeGraph(): Promise<Grafo> {
  return call<Grafo>('knowledge_graph')
}

/**
 * O texto de uma nota, para o painel lateral.
 *
 * Separado do grafo de propósito: mandar o corpo de todas as notas junto seria enviar a
 * base inteira pelo IPC para mostrar UMA.
 */
export function noteBody(id: string): Promise<string> {
  return call<string>('note_body', { id })
}
