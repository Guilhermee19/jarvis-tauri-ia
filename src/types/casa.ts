/** Espelho de `src-tauri/src/core/casa.rs`. Serde serializa em camelCase. */

/**
 * Um aparelho anunciado na rede local.
 *
 * Não tem `nome`: o anúncio que os aparelhos fazem na rede não carrega o nome que você
 * deu no app. Esse vem da nuvem da Tuya junto com a chave de controle, na próxima fase.
 */
export interface Aparelho {
  id: string
  ip: string
  /** "3.1", "3.3", "3.4", "3.5"… decide o que dá para usar para controlar. */
  versao: string
  produto: string | null
  /** `false` costuma ser aparelho novo, ainda esperando configuração no app. */
  ativo: boolean
  /** `false` quando o anúncio veio em texto puro (3.1) ou quando não abriu. */
  decifrado: boolean
  /**
   * Se dá para MANDAR COMANDO nele pelo caminho que o Jarvis fala.
   *
   * `false` no 3.5: o anúncio dele é lido (id, modelo, versão), mas o controle usa outro
   * quadro. O aparelho aparece na lista de qualquer jeito.
   */
  suportado: boolean
}

export interface Varredura {
  aparelhos: Aparelho[]
  /**
   * Pacotes que chegaram e não viraram aparelho.
   *
   * Existe para separar dois silêncios que dão a mesma tela vazia: **ninguém falou**
   * (rede errada, firewall) e **falaram e não entendi** (formato desconhecido). As
   * soluções são opostas, e sem esse número as duas parecem defeito de rede.
   */
  ignorados: number
}
