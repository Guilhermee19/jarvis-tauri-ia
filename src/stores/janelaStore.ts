import { create } from 'zustand'

import type { PanelPosition, PanelSize } from '@/components/ui/FloatingPanel'

/**
 * Onde o arranjo das janelas fica guardado entre execuções.
 *
 * `localStorage` e não o `settings.json`: isto não é configuração que você escolhe, é
 * onde você largou uma janela. O Rust nunca precisa ler, e passar cada pixel de arrasto
 * pelo IPC para gravar em disco seria caro para uma informação que só a interface usa.
 */
const CHAVE = 'jarvis:janelas'

/**
 * Quanto tempo esperar antes de gravar o arranjo.
 *
 * Arrastar uma janela dispara uma mudança de posição por quadro. Sem esperar o gesto
 * terminar, um arrasto de dois segundos escreveria cem vezes no `localStorage`.
 */
const ESPERA_MS = 500

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
export type JanelaId =
  | 'chat'
  | 'casa'
  | 'cameras'
  | 'desempenho'
  | 'navegador'
  | 'conhecimento'
  | 'musica'
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

/** Onde uma janelinha estava, e de que tamanho. */
export interface Arranjo {
  posicao: PanelPosition | null
  tamanho: PanelSize | null
  maximizada: boolean
}

const ARRANJO_NOVO: Arranjo = { posicao: null, tamanho: null, maximizada: false }

interface Guardado {
  fixadas: JanelaId[]
  arranjos: Partial<Record<JanelaId, Arranjo>>
}

interface JanelaState {
  /** Abertas, **do fundo para a frente** — a última é a que está por cima. */
  abertas: JanelaId[]
  gaveta: GavetaId | null
  /**
   * As que reabrem sozinhas ao subir o app, onde e do tamanho que ficaram.
   *
   * Fixar não impede de fechar: é uma preferência de abertura, não uma tranca. Fechar
   * uma janela fixada a tira da tela agora e ela volta na próxima vez — que é o
   * comportamento que não surpreende.
   */
  fixadas: JanelaId[]
  /**
   * Posição, tamanho e maximização de cada janelinha.
   *
   * Mora aqui e não num `useState` de cada componente porque agora precisa sobreviver ao
   * fechamento do APP, e não só ao da janela. Como efeito colateral, sumiram três cópias
   * do mesmo trio de estados.
   */
  arranjos: Partial<Record<JanelaId, Arranjo>>

  /**
   * Lê o que ficou guardado e abre as fixadas.
   *
   * Chamado de um efeito, e **não na criação da store**: o Next pré-renderiza a página
   * no build, onde não existe `localStorage`. Se a store nascesse já com janelas abertas
   * no navegador e fechadas no HTML gerado, o React acusaria divergência de hidratação —
   * e o conserto seria justamente adiar para depois da montagem, que é isto aqui.
   *
   * Idempotente: montar duas vezes (o modo estrito do React faz isso) não duplica nada.
   */
  hidratar: () => void
  fixar: (id: JanelaId, fixada: boolean) => void
  ajustar: (id: JanelaId, mudanca: Partial<Arranjo>) => void

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

/**
 * Lê o que ficou guardado.
 *
 * Tudo dentro de um `try`: `localStorage` lança em janela anônima e em navegador com
 * dados de site bloqueados, e um arranjo de janelas não pode impedir o app de subir.
 */
function carregar(): Guardado {
  const vazio: Guardado = { fixadas: [], arranjos: {} }
  if (typeof window === 'undefined') return vazio

  try {
    const cru = window.localStorage.getItem(CHAVE)
    if (!cru) return vazio

    const lido = JSON.parse(cru) as Partial<Guardado>

    return {
      fixadas: Array.isArray(lido.fixadas) ? lido.fixadas : [],
      arranjos: typeof lido.arranjos === 'object' && lido.arranjos !== null ? lido.arranjos : {},
    }
  } catch {
    // Guardado corrompido vira "nada guardado". A alternativa seria não abrir.
    return vazio
  }
}

let relogioDeGravacao: ReturnType<typeof setTimeout> | null = null

function guardar(estado: Guardado) {
  if (typeof window === 'undefined') return

  if (relogioDeGravacao !== null) clearTimeout(relogioDeGravacao)
  relogioDeGravacao = setTimeout(() => {
    try {
      window.localStorage.setItem(CHAVE, JSON.stringify(estado))
    } catch {
      // Sem espaço ou sem permissão: perde-se a memória entre sessões, e nada mais.
    }
  }, ESPERA_MS)
}

export const useJanelaStore = create<JanelaState>((set) => ({
  abertas: [],
  gaveta: null,
  fixadas: [],
  arranjos: {},

  hidratar: () =>
    set((state) => {
      const guardado = carregar()

      return {
        fixadas: guardado.fixadas,
        arranjos: guardado.arranjos,
        // As fixadas nascem abertas: é literalmente o que fixar quer dizer. A ordem da
        // lista guardada é preservada, então a que estava na frente continua na frente —
        // e o que já estiver aberto não é fechado nem duplicado.
        abertas: [
          ...state.abertas.filter((atual) => !guardado.fixadas.includes(atual)),
          ...guardado.fixadas,
        ],
      }
    }),

  fixar: (id, fixada) =>
    set((state) => {
      const fixadas = fixada
        ? [...state.fixadas.filter((atual) => atual !== id), id]
        : state.fixadas.filter((atual) => atual !== id)

      guardar({ fixadas, arranjos: state.arranjos })

      return { fixadas }
    }),

  ajustar: (id, mudanca) =>
    set((state) => {
      const arranjos = {
        ...state.arranjos,
        [id]: { ...(state.arranjos[id] ?? ARRANJO_NOVO), ...mudanca },
      }

      // Só as fixadas vão para o disco: quem não pediu para reabrir não tem por que
      // deixar rastro, e arrastar uma janela qualquer não deveria escrever nada.
      if (state.fixadas.includes(id)) guardar({ fixadas: state.fixadas, arranjos })

      return { arranjos }
    }),

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
