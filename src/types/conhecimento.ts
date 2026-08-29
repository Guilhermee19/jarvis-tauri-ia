/** Espelha `src-tauri/src/core/memory/grafo.rs` (serde em camelCase). */

/** Um assunto que o Jarvis conhece — uma nota da pasta `memoria/notas/`. */
export interface NoDoGrafo {
  /** O slug da nota. É a chave, e o alvo de `[[isto]]`. */
  id: string
  /** O mesmo, legível: `tony-stark` vira `Tony Stark`. */
  rotulo: string
  /** `fato`, `aprendido`, `resumo` ou `rotina` — é por aqui que a tela filtra. */
  tipo: string
  /** Quanto ele sabe do assunto, de 0 a 1. Vira o tamanho do círculo. */
  peso: number
  /** Caracteres da nota. Vai no painel lateral, porque "0,73" sozinho não explica nada. */
  tamanho: number
  atualizado: string
  /** Quantas outras notas apontam para esta. */
  citacoes: number
}

/** Uma ligação entre dois assuntos. */
export interface ArestaDoGrafo {
  de: string
  para: string
  /** 0 a 1. Nas escritas é sempre 1; nas inferidas é a semelhança medida. */
  forca: number
  /**
   * `true` quando o Jarvis ESCREVEU `[[link]]` na nota.
   *
   * A tela desenha essas cheias e as inferidas apagadas. A distinção não é decoração:
   * uma é uma relação que ele afirmou, a outra é um palpite por vocabulário em comum, e
   * desenhar as duas iguais apresentaria o palpite como fato.
   */
  escrita: boolean
}

export interface Grafo {
  nos: NoDoGrafo[]
  arestas: ArestaDoGrafo[]
}

/**
 * Os tipos de nota, na ordem do filtro.
 *
 * São os que o Rust já grava no frontmatter — procedência, não assunto. Um filtro por
 * tema ("Projetos", "Tecnologias") precisaria de um campo que hoje não existe nos dados.
 */
export const TIPOS_DE_NOTA = [
  { id: 'fato', rotulo: 'Fatos' },
  { id: 'aprendido', rotulo: 'Aprendidos' },
  { id: 'resumo', rotulo: 'Resumos' },
  { id: 'rotina', rotulo: 'Rotinas' },
] as const
