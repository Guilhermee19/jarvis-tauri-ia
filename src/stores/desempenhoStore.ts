import { create } from 'zustand'
import { performanceMetrics } from '@/lib/tauri'
import type { Metricas } from '@/types'

/**
 * De quanto em quanto tempo perguntar ao Windows.
 *
 * Um segundo é o intervalo em que o próprio Gerenciador de Tarefas trabalha, e não é
 * coincidência: os contadores de taxa medem a distância entre duas leituras, então ler
 * mais rápido não dá mais informação — dá a mesma informação com mais ruído.
 */
const INTERVALO_MS = 1000

/**
 * Quantas amostras o gráfico guarda.
 *
 * 60 com o intervalo de 1 s é um minuto de história, que é o suficiente para ver um pico
 * acontecer e passar. Mais que isso viraria uma linha achatada em poucos pixels.
 */
const HISTORICO = 60

/**
 * Uso de processador, memória e placa de vídeo.
 *
 * A leitura vive numa store e não no componente porque ela tem MEMÓRIA: o gráfico é feito
 * do histórico, e fechar e reabrir o painel jogaria fora o minuto que já foi observado.
 * O laço, esse sim, só roda com o painel aberto — ninguém precisa de amostras de uma tela
 * que não está na frente.
 */
interface DesempenhoState {
  atual: Metricas | null
  /** As últimas [`HISTORICO`] amostras, da mais velha para a mais nova. */
  cpu: number[]
  gpu: number[]
  memoria: number[]
  erro: string | null
  iniciar: () => void
  parar: () => void
}

function descrever(erro: unknown): string {
  return erro instanceof Error ? erro.message : String(erro)
}

function empurrar(serie: number[], valor: number): number[] {
  return [...serie, valor].slice(-HISTORICO)
}

/**
 * O relógio do laço.
 *
 * Fora do estado do zustand de propósito: ele não é dado que a tela desenha, e pôr um id
 * de timer no estado faria toda batida notificar todo componente inscrito.
 */
let relogio: ReturnType<typeof setInterval> | null = null

export const useDesempenhoStore = create<DesempenhoState>((set, get) => {
  async function amostrar() {
    try {
      const metricas = await performanceMetrics()

      set({
        atual: metricas,
        cpu: empurrar(get().cpu, metricas.cpu),
        gpu: empurrar(get().gpu, metricas.gpu),
        memoria: empurrar(
          get().memoria,
          metricas.memoriaTotal > 0 ? (metricas.memoriaUsada / metricas.memoriaTotal) * 100 : 0,
        ),
        erro: null,
      })
    } catch (erro) {
      // Para o laço na primeira falha: se os contadores não abriram, eles não vão abrir
      // na tentativa seguinte, e repetir o mesmo erro uma vez por segundo é só barulho.
      get().parar()
      set({ erro: descrever(erro) })
    }
  }

  return {
    atual: null,
    cpu: [],
    gpu: [],
    memoria: [],
    erro: null,

    iniciar: () => {
      if (relogio !== null) return

      set({ erro: null })
      void amostrar()
      relogio = setInterval(() => void amostrar(), INTERVALO_MS)
    },

    parar: () => {
      if (relogio === null) return

      clearInterval(relogio)
      relogio = null
    },
  }
})
