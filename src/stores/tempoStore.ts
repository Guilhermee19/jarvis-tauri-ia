import { create } from 'zustand'

import type { Previsao } from '@/types'

/**
 * A previsão que está no card.
 *
 * **Só guarda o que o Rust mandou — não busca nada.** Gêmeo do `cotacoesStore`, e pela
 * mesma razão: quem fala com a Open-Meteo é o agente, e ele manda os números junto do
 * pedido de abrir o card (`AcaoDeUi::Tempo`). Consultar de novo aqui faria duas idas à
 * rede por pergunta e deixaria a tela mostrando um instante diferente do que foi falado.
 *
 * A consequência é que o card é o retrato de quando ele respondeu, e é por isso que o
 * rodapé diz a hora: previsão sem hora parece ao vivo.
 */
interface TempoState {
  previsao: Previsao | null
  /** O lugar, já escrito por extenso. Vazio quer dizer "aqui" — ver `AcaoDeUi::Tempo`. */
  lugar: string
  /** Quando o card foi preenchido, em epoch ms. */
  atualizado: number | null

  definir: (lugar: string, previsao: Previsao) => void
  limpar: () => void
}

export const useTempoStore = create<TempoState>((set) => ({
  previsao: null,
  lugar: '',
  atualizado: null,

  // A hora vem do relógio local, e não da fonte: ao contrário da cotação, a previsão não
  // carrega um "quando" próprio — o que interessa aqui é há quanto tempo ELE olhou.
  definir: (lugar, previsao) => set({ lugar, previsao, atualizado: Date.now() }),

  limpar: () => set({ previsao: null, lugar: '', atualizado: null }),
}))
