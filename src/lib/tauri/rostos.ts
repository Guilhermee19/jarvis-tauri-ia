import type { PessoaConhecida, QuemEsta } from '@/types'
import { call } from './client'

/** Wrappers de `src-tauri/src/commands/rostos.rs`. */

/**
 * Olha pela webcam e diz quem está lá.
 *
 * **Tira uma foto e fecha a câmera** — a luz pisca em vez de ficar acesa. Com o preview
 * já ligado, aproveita a sessão aberta e não interfere nela.
 *
 * Leva ~1,3 s com a câmera fria (abrir o dispositivo domina o tempo) e ~50 ms com ela já
 * aberta. Não reconhecer ninguém **não é erro**: volta com `nome` vazio.
 */
export function whoIsThere(): Promise<QuemEsta> {
  return call<QuemEsta>('who_is_there')
}

/**
 * Guarda o rosto que está na câmera agora sob um nome.
 *
 * Chamar de novo com o mesmo nome **acrescenta** uma condição em vez de substituir — é o
 * que faz reconhecer de óculos sem deixar de reconhecer sem.
 */
export function enrollFace(nome: string): Promise<PessoaConhecida> {
  return call<PessoaConhecida>('enroll_face', { nome })
}

export function listPeople(): Promise<PessoaConhecida[]> {
  return call<PessoaConhecida[]>('list_people')
}

export function forgetPerson(id: string): Promise<void> {
  return call<void>('forget_person', { id })
}
