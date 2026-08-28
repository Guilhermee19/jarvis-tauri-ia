import { create } from 'zustand'

/**
 * O que está na tela, e em que ordem.
 *
 * Duas famílias com regras diferentes, e a diferença é física:
 *
 * - **Janelinhas** (conversa, casa, música) flutuam sobre o HUD, se movem e **convivem**.
 *   Ter a lista da casa aberta num canto enquanto se conversa é uso normal, não conflito.
 * - **Gavetas** (configurações) ocupam a faixa da borda direita.
 *
 * Antes disto o store guardava UM painel ativo para os dois casos, e abrir a casa fechava
 * a conversa — que é o comportamento certo para gaveta e errado para janela.
 */
export type JanelaId = 'chat' | 'casa' | 'musica'
export type GavetaId = 'settings'

/**
 * z-index da janela mais ao fundo.
 *
 * A faixa **30–39 é das janelinhas** — acima do HUD, que é fundo, e abaixo das gavetas
 * (`z-40` no `Sheet`), que são temporárias e devem cobrir o que estiver embaixo. Como o
 * empilhamento soma o índice, a faixa comporta dez janelas antes de encostar na gaveta,
 * e hoje existem três.
 */
const Z_BASE = 30

interface JanelaState {
  /** Abertas, **do fundo para a frente** — a última é a que está por cima. */
  abertas: JanelaId[]
  gaveta: GavetaId | null

  /** Abre; se já estava aberta, só traz para a frente. */
  abrir: (id: JanelaId) => void
  fechar: (id: JanelaId) => void
  /**
   * Semântica de barra de tarefas, igual à do Windows: fechada **abre**, atrás de outra
   * **vem para a frente**, e já na frente **fecha**. Sem o passo do meio, clicar no
   * ícone de uma janela que está escondida atrás de outra a fecharia — que é o oposto
   * do que a pessoa quis ao clicar.
   */
  alternar: (id: JanelaId) => void

  abrirGaveta: (id: GavetaId) => void
  fecharGaveta: () => void
  alternarGaveta: (id: GavetaId) => void
}

export const useJanelaStore = create<JanelaState>((set) => ({
  abertas: [],
  gaveta: null,

  abrir: (id) =>
    set((state) => ({ abertas: [...state.abertas.filter((atual) => atual !== id), id] })),

  fechar: (id) => set((state) => ({ abertas: state.abertas.filter((atual) => atual !== id) })),

  alternar: (id) =>
    set((state) => {
      const naFrente = state.abertas[state.abertas.length - 1] === id
      if (naFrente) return { abertas: state.abertas.filter((atual) => atual !== id) }

      return { abertas: [...state.abertas.filter((atual) => atual !== id), id] }
    }),

  abrirGaveta: (id) => set({ gaveta: id }),
  fecharGaveta: () => set({ gaveta: null }),
  alternarGaveta: (id) => set((state) => ({ gaveta: state.gaveta === id ? null : id })),
}))

/**
 * O `z-index` de uma janela, derivado da posição dela na pilha.
 *
 * Função pura em vez de campo no estado: a ordem já é a verdade, e guardar um número por
 * janela ao lado dela seria a mesma informação em dois lugares — com a chance de
 * discordarem.
 *
 * Vai no `style` e não numa classe do Tailwind porque o valor é calculado: o JIT só gera
 * as classes que encontra escritas no código, e `z-${n}` não existiria no CSS final.
 */
export function zDaJanela(abertas: JanelaId[], id: JanelaId): number {
  const posicao = abertas.indexOf(id)
  return posicao === -1 ? Z_BASE : Z_BASE + posicao
}
