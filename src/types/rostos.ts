/**
 * Quem o Jarvis conhece de rosto. Espelho manual de `core::rostos` no Rust.
 */

/** O que a saudação recebe ao perguntar "quem está aí?". Espelha `commands::rostos::QuemEsta`. */
export interface QuemEsta {
  /**
   * O nome, quando reconhecido. **Vazio quando não** — e é o estado que interessa,
   * porque é ele que faz o Jarvis saudar sem arriscar nome nenhum.
   */
  nome: string
  id: string
  /** De 0 a 1. Zero quando não reconheceu ninguém. */
  semelhanca: number
  /**
   * Havia alguém na frente da câmera, mesmo que desconhecido.
   *
   * Separa dois silêncios que o `nome` vazio junta: "não tem ninguém aí" e "tem alguém
   * que eu não conheço". Só o segundo merece perguntar quem é — não se pergunta a uma
   * cadeira vazia.
   */
  temAlguem: boolean
}

/** Uma pessoa cadastrada. Espelha `commands::rostos::PessoaConhecida`. */
export interface PessoaConhecida {
  id: string
  nome: string
  /** Quantas condições diferentes foram cadastradas (de óculos, de manhã, de barba). */
  cadastros: number
  /** Quando foi visto pela última vez, em ms. `0` = nunca desde o cadastro. */
  vistoEm: number
}
