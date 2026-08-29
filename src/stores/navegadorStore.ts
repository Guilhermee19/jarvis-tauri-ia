import { create } from 'zustand'
import {
  browserBounds,
  browserClose,
  browserHistory,
  browserNavigate,
  browserOpen,
  browserSearch,
  browserSelect,
} from '@/lib/tauri'
import type { Aba, AreaDoNavegador } from '@/types'
import { useJanelaStore } from './janelaStore'

/**
 * As abas do navegador interno.
 *
 * **O estado de verdade mora no Rust**, porque lá é que vivem os webviews — esta store é
 * um espelho. Toda ação devolve o retrato inteiro e substitui o que havia aqui: manter
 * uma cópia própria e tentar mantê-la em dia daria duas verdades, e a de cá ficaria para
 * trás no primeiro fechamento de aba.
 */
interface NavegadorState {
  abas: Aba[]
  ativa: string | null
  erro: string | null
  /** `true` enquanto uma aba está sendo aberta — abrir demora o tempo de criar o webview. */
  abrindo: boolean

  abrirSite: (url: string) => Promise<void>
  pesquisar: (termo: string) => Promise<void>
  selecionar: (id: string) => Promise<void>
  fechar: (id: string) => Promise<void>
  navegar: (id: string, url: string) => Promise<void>
  andar: (id: string, passo: number) => Promise<void>
  /** Informa onde desenhar. `null` esconde tudo. */
  posicionar: (area: AreaDoNavegador | null) => void
  limparErro: () => void
}

function descrever(erro: unknown): string {
  return erro instanceof Error ? erro.message : String(erro)
}

/**
 * A última área enviada, para não repetir a mesma chamada.
 *
 * Fora do estado do zustand: arrastar o painel dispara uma medida por quadro, e cada uma
 * delas notificaria todo componente inscrito para nada.
 */
let ultimaArea: string | null = null

export const useNavegadorStore = create<NavegadorState>((set) => {
  /** Toda ação termina igual: guarda o retrato que o Rust devolveu. */
  async function aplicar(acao: Promise<{ abas: Aba[]; ativa: string | null }>) {
    try {
      const estado = await acao
      set({ abas: estado.abas, ativa: estado.ativa, erro: null })
    } catch (erro) {
      set({ erro: descrever(erro) })
    }
  }

  return {
    abas: [],
    ativa: null,
    erro: null,
    abrindo: false,

    abrirSite: async (url) => {
      // A janela precisa estar aberta ANTES do webview nascer: é ela que mede o buraco e
      // diz onde desenhar. Sem isso a aba nasce fora da tela e só aparece no primeiro
      // movimento do painel.
      useJanelaStore.getState().abrir('navegador')
      set({ abrindo: true })
      await aplicar(browserOpen(url))
      set({ abrindo: false })
    },

    pesquisar: async (termo) => {
      useJanelaStore.getState().abrir('navegador')
      set({ abrindo: true })
      await aplicar(browserSearch(termo))
      set({ abrindo: false })
    },

    selecionar: (id) => aplicar(browserSelect(id)),
    fechar: (id) => aplicar(browserClose(id)),
    navegar: (id, url) => aplicar(browserNavigate(id, url)),

    andar: async (id, passo) => {
      try {
        await browserHistory(id, passo)
      } catch (erro) {
        set({ erro: descrever(erro) })
      }
    },

    posicionar: (area) => {
      // Comparação por texto para não mandar a mesma área duas vezes: o painel dispara
      // uma medida por quadro enquanto é arrastado, e cada chamada dessas atravessa o IPC
      // e mexe numa janela nativa.
      const chave = area === null ? 'oculto' : JSON.stringify(area)
      if (chave === ultimaArea) return
      ultimaArea = chave

      void browserBounds(area).catch((erro: unknown) => set({ erro: descrever(erro) }))
    },

    limparErro: () => set({ erro: null }),
  }
})

/** A aba que está na frente, ou `undefined` com o navegador vazio. */
export function abaAtiva(estado: NavegadorState): Aba | undefined {
  return estado.abas.find((aba) => aba.id === estado.ativa)
}
