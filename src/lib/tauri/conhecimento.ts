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

/**
 * Reescreve uma nota com o texto corrigido.
 *
 * O que ele aprende sozinho erra — a extração automática é best-effort, e uma busca já
 * virou nota sobre o assunto errado. Sem isto, corrigir exigia achar o `.md` na pasta.
 *
 * O TIPO da nota não muda: ele diz de onde o conhecimento veio, e passar a mão no texto
 * não reescreve a origem. A data de atualização passa a ser hoje.
 */
export function saveNote(id: string, corpo: string): Promise<void> {
  return call<void>('save_note', { id, corpo })
}

/**
 * Apaga uma nota — a que está aberta, e só ela.
 *
 * Diferente do "esquece X" falado, que casa por termo e pode levar várias notas junto.
 * Para um botão, casar por termo seria uma armadilha.
 */
export function deleteNote(id: string): Promise<void> {
  return call<void>('delete_note', { id })
}
