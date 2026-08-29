/** Espelha `src-tauri/src/navegador.rs` (serde em camelCase). */

/**
 * Uma aba do navegador interno.
 *
 * **Não é um `<iframe>`.** Cada aba é um webview filho da janela — o mesmo motor do resto
 * do app. Foi medido que Google, YouTube e DuckDuckGo respondem `X-Frame-Options:
 * SAMEORIGIN` e ficariam em branco num iframe, e "abre o youtube" é justamente o exemplo
 * canônico do roteador de intenção.
 */
export interface Aba {
  id: string
  url: string
  /**
   * O que aparece na lingueta: o host, sem `www.`.
   *
   * Não é o `<title>` da página. Pegá-lo exigiria injetar JavaScript e esperar a resposta,
   * e "youtube.com" já identifica a aba no instante em que ela nasce.
   */
  titulo: string
}

/** O retrato do navegador depois de qualquer mexida. */
export interface EstadoDoNavegador {
  abas: Aba[]
  ativa: string | null
}

/**
 * Onde as abas são desenhadas, em pixels lógicos da janela.
 *
 * Existe porque o webview é uma camada **nativa** acima do HTML: nenhum CSS o posiciona, e
 * sem alguém medir o buraco do painel e contar para o Rust, ele nunca aparece no lugar
 * certo.
 */
export interface AreaDoNavegador {
  x: number
  y: number
  largura: number
  altura: number
}
