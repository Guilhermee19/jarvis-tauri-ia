/** Espelha `Metricas` de `src-tauri/src/core/system/desempenho.rs` (serde em camelCase). */

/**
 * Um retrato do computador neste instante.
 *
 * Bytes crus e não texto formatado: quem desenha a barra precisa da proporção, e quem
 * escreve "3,2 GB" precisa decidir a unidade junto com o espaço que tem na tela.
 */
export interface Metricas {
  /** 0 a 100. */
  cpu: number
  memoriaUsada: number
  memoriaTotal: number
  /**
   * 0 a 100. É a soma dos motores da placa (3D, cópia, codificação de vídeo…), limitada
   * a 100 — a mesma conta do Gerenciador de Tarefas.
   */
  gpu: number
  /** `0` quando a placa não expõe o contador, o que é comum em vídeo integrado. */
  gpuMemoriaUsada: number
  gpuMemoriaTotal: number
  /** O nome da placa, para o cartão não dizer só "GPU". */
  gpuNome: string
}
