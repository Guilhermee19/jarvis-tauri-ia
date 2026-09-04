import { create } from 'zustand'

import type { Cotacao } from '@/types'

/**
 * As cotações que estão no card.
 *
 * **Só guarda o que o Rust mandou — não busca nada.** Quem fala com a AwesomeAPI é o
 * agente, e ele já manda os números junto do pedido de abrir o card
 * (`AcaoDeUi::Cotacoes`). Buscar de novo aqui faria duas idas à rede por pergunta e, pior,
 * deixaria a fala e a tela mostrando números de instantes diferentes.
 *
 * A consequência é que o card mostra o retrato do momento em que ele respondeu, e é por
 * isso que o `quando` de cada cotação aparece na tela: um número sem hora parece ao vivo.
 */
interface CotacoesState {
  cotacoes: Cotacao[]
  /** Quando o card foi preenchido, para o rodapé poder dizer "agora" ou "às 22:40". */
  atualizado: string | null
  definir: (cotacoes: Cotacao[]) => void
  limpar: () => void
}

export const useCotacoesStore = create<CotacoesState>((set) => ({
  cotacoes: [],
  atualizado: null,

  definir: (cotacoes) =>
    set({
      cotacoes,
      // A hora vem da PRIMEIRA cotação, não do relógio local: o que interessa é quando a
      // fonte cotou, não quando o pacote chegou aqui.
      atualizado: cotacoes[0]?.quando ?? null,
    }),

  limpar: () => set({ cotacoes: [], atualizado: null }),
}))
