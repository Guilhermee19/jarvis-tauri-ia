import { useMemo } from 'react'
import { create } from 'zustand'
import { deleteNote, knowledgeGraph, noteBody, saveNote } from '@/lib/tauri'
import type { Grafo, NoDoGrafo } from '@/types'

/**
 * O mapa do que o Jarvis sabe.
 *
 * O grafo inteiro vem numa chamada só e fica aqui; o CORPO de cada nota vem sob demanda,
 * quando alguém clica num nó. Mandar todos os corpos junto seria a base de conhecimento
 * inteira atravessando o IPC para mostrar uma nota.
 */
interface ConhecimentoState {
  grafo: Grafo
  /** Tipos visíveis. Vazio quer dizer TODOS — é o estado inicial e o mais útil. */
  filtros: string[]
  /** O nó aberto no painel lateral, com o corpo já carregado. */
  aberto: { no: NoDoGrafo; corpo: string } | null
  busca: string
  carregando: boolean
  erro: string | null

  atualizar: () => Promise<void>
  abrir: (no: NoDoGrafo) => Promise<void>
  /**
   * Corrige o texto da nota aberta.
   *
   * Relê o grafo depois de gravar, e não é preciosismo: as arestas saem dos `[[links]]`
   * escritos no corpo, então mexer no texto pode criar ou cortar uma ligação — o desenho
   * ficaria mentindo até alguém clicar em atualizar.
   */
  salvar: (corpo: string) => Promise<boolean>
  /** Apaga a nota aberta e fecha o painel. Nota errada nem sempre é para corrigir. */
  apagar: () => Promise<void>
  fechar: () => void
  alternarFiltro: (tipo: string) => void
  buscar: (termo: string) => void
}

function descrever(erro: unknown): string {
  return erro instanceof Error ? erro.message : String(erro)
}

export const useConhecimentoStore = create<ConhecimentoState>((set, get) => ({
  grafo: { nos: [], arestas: [] },
  filtros: [],
  aberto: null,
  busca: '',
  carregando: false,
  erro: null,

  atualizar: async () => {
    set({ carregando: true })
    try {
      set({ grafo: await knowledgeGraph(), erro: null })
    } catch (erro) {
      set({ erro: descrever(erro) })
    } finally {
      set({ carregando: false })
    }
  },

  abrir: async (no) => {
    // O nó entra ANTES do corpo: o painel abre na hora com o que já se sabe, e o texto
    // preenche quando chega. Esperar o IPC deixaria o clique sem resposta.
    set({ aberto: { no, corpo: '' } })
    try {
      const corpo = await noteBody(no.id)
      // Só escreve se o painel ainda mostra o MESMO nó — dois cliques rápidos fariam a
      // resposta lenta do primeiro sobrescrever o segundo.
      if (get().aberto?.no.id === no.id) set({ aberto: { no, corpo } })
    } catch (erro) {
      set({ erro: descrever(erro) })
    }
  },

  salvar: async (corpo) => {
    const aberto = get().aberto
    if (!aberto) return false

    try {
      await saveNote(aberto.no.id, corpo)
    } catch (erro) {
      set({ erro: descrever(erro) })
      return false
    }

    // O painel mostra o texto novo na hora; o grafo relê porque as ligações podem ter
    // mudado com ele.
    const texto = corpo.trim()
    set({ aberto: { no: aberto.no, corpo: texto }, erro: null })
    await get().atualizar()

    // E o cabeçalho do painel se acerta com o nó recém-lido: tamanho, citações e data de
    // atualização mudaram com a edição, e mostrar os números velhos ao lado do texto novo
    // seria a tela mentindo sobre o que acabou de acontecer.
    const relido = get().grafo.nos.find((no) => no.id === aberto.no.id)
    if (relido) set({ aberto: { no: relido, corpo: texto } })

    return true
  },

  apagar: async () => {
    const aberto = get().aberto
    if (!aberto) return

    try {
      await deleteNote(aberto.no.id)
    } catch (erro) {
      set({ erro: descrever(erro) })
      return
    }

    // Fecha antes de reler: o nó não existe mais, e um painel apontando para ele ficaria
    // mostrando o texto de uma nota que já não está no grafo.
    set({ aberto: null, erro: null })
    await get().atualizar()
  },

  fechar: () => set({ aberto: null }),

  alternarFiltro: (tipo) =>
    set((estado) => ({
      filtros: estado.filtros.includes(tipo)
        ? estado.filtros.filter((atual) => atual !== tipo)
        : [...estado.filtros, tipo],
    })),

  buscar: (termo) => set({ busca: termo }),
}))

/**
 * O grafo depois do filtro e da busca.
 *
 * **Hook, e não seletor do zustand** — e isso não é estilo, é a correção de um bug. Como
 * ele monta um objeto novo (`{ nos, arestas }`) a cada chamada, usá-lo como seletor fazia
 * o zustand comparar referências diferentes a cada render com `Object.is`, renderizar de
 * novo, montar outro objeto, e assim por diante. O React acusa isso como
 * "The result of getSnapshot should be cached" e depois derruba com
 * "Maximum update depth exceeded".
 *
 * O `useMemo` sobre as três fontes resolve porque as três SÃO estáveis: `grafo` só muda
 * quando as notas são relidas, `filtros` só quando alguém clica, e `busca` é uma string.
 */
export function useGrafoVisivel(): Grafo {
  const grafo = useConhecimentoStore((estado) => estado.grafo)
  const filtros = useConhecimentoStore((estado) => estado.filtros)
  const busca = useConhecimentoStore((estado) => estado.busca)

  return useMemo(() => filtrar(grafo, filtros, busca), [grafo, filtros, busca])
}

/**
 * O filtro em si, puro e testável.
 *
 * As arestas são refeitas junto: uma aresta cujo nó sumiu viraria uma linha para o nada, e
 * a biblioteca de desenho trata isso como erro em vez de ignorar.
 */
function filtrar(grafo: Grafo, filtros: string[], busca: string): Grafo {
  const termo = busca.trim().toLowerCase()

  const nos = grafo.nos.filter((no) => {
    const tipoOk = filtros.length === 0 || filtros.includes(no.tipo)
    const buscaOk = termo === '' || no.rotulo.toLowerCase().includes(termo)
    return tipoOk && buscaOk
  })

  const vivos = new Set(nos.map((no) => no.id))

  return {
    nos,
    arestas: grafo.arestas.filter((aresta) => vivos.has(aresta.de) && vivos.has(aresta.para)),
  }
}
