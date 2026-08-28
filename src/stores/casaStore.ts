import { create } from 'zustand'
import {
  deviceState,
  discoverDevices,
  importTuyaDevices,
  knownDevices,
  setDeviceHidden,
  setDevicePower,
  setLight,
} from '@/lib/tauri'
import type { AjusteLuz, Aparelho, DetalheAparelho } from '@/types'
import { useSettingsStore } from './settingsStore'

/** De quanto em quanto tempo a ronda repete, enquanto o painel estiver aberto. */
const RONDA_MS = 30_000

/**
 * Os aparelhos da casa encontrados na rede.
 *
 * Mora numa store, e não no componente, porque a varredura leva segundos: fechar e
 * reabrir o painel no meio dela jogaria fora uma espera que já foi paga. O resultado
 * sobrevive ao painel; a busca, não é reiniciada por ele.
 *
 * A lista vem do Rust já misturando duas coisas: quem anunciou AGORA (`presente`) e quem
 * já anunciou um dia e está calado (`presente: false`). Um aparelho fora da tomada
 * continua na tela, marcado — some da lista seria pior, porque manda procurar defeito no
 * Wi-Fi quando o problema é a tomada.
 */
interface CasaState {
  aparelhos: Aparelho[]
  procurando: boolean
  /** `null` antes da primeira busca — que é diferente de "buscou e não achou nada". */
  buscouEm: number | null
  /**
   * Pacotes que chegaram na rede sem virarem aparelho.
   *
   * Separa dois silêncios que dão a mesma tela vazia e têm soluções opostas: ninguém
   * falou (rede errada, firewall) e falaram num formato que não sabemos ler.
   */
  ignorados: number
  erro: string | null
  procurar: () => Promise<void>
  /**
   * Quantos aparelhos a última importação da nuvem trouxe. `null` = ainda não importou
   * nesta sessão — que é diferente de "importou e não veio nada".
   */
  importados: number | null
  importando: boolean
  /**
   * `true` quando a última importação falhou.
   *
   * Existe para a ronda não repetir de 30 em 30 segundos um erro cuja causa é uma
   * configuração no site da Tuya — que não se conserta sozinha. O botão manual ignora
   * esta trava, porque clicar nele é justamente dizer "arrumei, tenta de novo".
   */
  importFalhou: boolean
  importar: (automatico?: boolean) => Promise<void>
  /**
   * O último estado CONFIRMADO de cada aparelho, por id.
   *
   * Vazio até alguém mandar um comando, e é assim de propósito: saber o estado exige uma
   * conexão TCP por aparelho, e pagar três delas a cada varredura para preencher uma
   * bolinha na tela não se justifica. O que está aqui foi confirmado pelo aparelho, não
   * presumido.
   */
  estados: Record<string, boolean>
  /** O id do aparelho que está no meio de um comando. Um por vez basta. */
  comandando: string | null
  alternar: (aparelho: Aparelho, ligado: boolean) => Promise<void>
  /**
   * O retrato técnico de cada aparelho, por id — data points, capacidades de luz.
   *
   * Só é buscado quando alguém ABRE os detalhes: custa uma conexão TCP e um aperto de
   * mão por aparelho, e pagar isso a cada varredura para preencher uma tela que ninguém
   * está olhando seria desperdício.
   */
  detalhes: Record<string, DetalheAparelho>
  /** O id do aparelho cujos detalhes estão sendo buscados ou ajustados. */
  detalhando: string | null
  detalhar: (aparelho: Aparelho) => Promise<void>
  ajustarLuz: (aparelho: Aparelho, ajuste: AjusteLuz) => Promise<void>
  /** Tira da lista principal, ou devolve para ela. Só a tela muda. */
  ocultar: (aparelho: Aparelho, oculto: boolean) => Promise<void>
  /** Mostra na hora o que já se conhece, sem esperar os 10 s da varredura. */
  carregar: () => Promise<void>
  /**
   * Liga e desliga a ronda de 30 em 30 segundos, que mantém a presença em dia e pega
   * aparelho novo sem ninguém clicar em nada.
   */
  iniciarRonda: () => void
  pararRonda: () => void
}

/**
 * O relógio da ronda.
 *
 * Fora do estado do zustand de propósito: ele não é dado que a tela desenha, e pôr um
 * id de timer no estado faria toda batida da ronda notificar todo componente inscrito.
 */
let ronda: ReturnType<typeof setInterval> | null = null

/**
 * O id que a busca na nuvem usa como ponto de partida.
 *
 * A Tuya lista aparelhos de um USUÁRIO, e o usuário se descobre perguntando por um
 * aparelho conhecido — qualquer um serve. Os que não foram decifrados ficam de fora
 * porque o "id" deles é um endereço inventado por nós, não um id da Tuya.
 */
function semente(aparelhos: Aparelho[]): string {
  return aparelhos.find((aparelho) => !aparelho.id.startsWith('desconhecido@'))?.id ?? ''
}

function descrever(erro: unknown): string {
  return erro instanceof Error ? erro.message : String(erro)
}

export const useCasaStore = create<CasaState>((set, get) => ({
  aparelhos: [],
  procurando: false,
  buscouEm: null,
  ignorados: 0,
  erro: null,
  importados: null,
  importando: false,
  importFalhou: false,
  estados: {},
  comandando: null,
  detalhes: {},
  detalhando: null,

  ocultar: async (aparelho, oculto) => {
    // A lista muda ANTES do disco: esconder um cartão é um gesto de interface, e esperar
    // uma gravação para ele sumir faria o clique parecer que não pegou.
    set({
      aparelhos: get().aparelhos.map((atual) =>
        atual.id === aparelho.id ? { ...atual, oculto } : atual,
      ),
    })

    try {
      await setDeviceHidden(aparelho.id, oculto)
    } catch (erro) {
      set({ erro: descrever(erro) })
    }
  },

  detalhar: async (aparelho) => {
    if (get().detalhando !== null) return
    set({ detalhando: aparelho.id, erro: null })

    try {
      const detalhe = await deviceState(aparelho.id, aparelho.ip, aparelho.versao)
      set({
        detalhes: { ...get().detalhes, [aparelho.id]: detalhe },
        estados: { ...get().estados, [aparelho.id]: detalhe.ligado },
      })
    } catch (erro) {
      set({ erro: descrever(erro) })
    } finally {
      set({ detalhando: null })
    }
  },

  ajustarLuz: async (aparelho, ajuste) => {
    if (get().detalhando !== null) return
    set({ detalhando: aparelho.id, erro: null })

    try {
      // O Rust relê a lâmpada depois de aplicar, então o que volta é como ela FICOU e
      // não o que foi pedido — ela arredonda valores e recusa combinações.
      const detalhe = await setLight(aparelho.id, aparelho.ip, aparelho.versao, ajuste)
      set({
        detalhes: { ...get().detalhes, [aparelho.id]: detalhe },
        estados: { ...get().estados, [aparelho.id]: detalhe.ligado },
      })
    } catch (erro) {
      set({ erro: descrever(erro) })
    } finally {
      set({ detalhando: null })
    }
  },

  carregar: async () => {
    try {
      const conhecidos = await knownDevices()
      // Só semeia a tela: uma varredura em andamento já tem a verdade mais nova, e
      // sobrescrevê-la com fichas velhas seria andar para trás.
      if (conhecidos.length > 0 && get().aparelhos.length === 0) {
        set({ aparelhos: conhecidos })
      }
    } catch {
      // Silêncio de propósito: isto é um atalho de exibição. Se falhar, a varredura
      // logo em seguida traz tudo, e um erro aqui só assustaria à toa.
    }
  },

  procurar: async () => {
    if (get().procurando) return
    set({ procurando: true, erro: null })

    try {
      const varredura = await discoverDevices()
      // O Rust já devolve a união do que está na rede agora com o que ficou anotado, e
      // por isso substituir a lista inteira não perde ninguém.
      set({
        aparelhos: varredura.aparelhos,
        ignorados: varredura.ignorados,
        buscouEm: Date.now(),
      })

      // Achar e dar nome são um gesto só. Pedir um clique a mais depois de esperar dez
      // segundos era transformar meia tarefa em duas.
      await get().importar(true)
    } catch (erro) {
      set({ erro: descrever(erro) })
    } finally {
      set({ procurando: false })
    }
  },

  iniciarRonda: () => {
    if (ronda !== null) return

    void get().carregar()
    void get().procurar()
    ronda = setInterval(() => void get().procurar(), RONDA_MS)
  },

  pararRonda: () => {
    if (ronda === null) return

    clearInterval(ronda)
    ronda = null
  },

  alternar: async (aparelho, ligado) => {
    if (get().comandando !== null) return
    set({ comandando: aparelho.id, erro: null })

    try {
      const estado = await setDevicePower(aparelho.id, aparelho.ip, aparelho.versao, ligado)
      set({ estados: { ...get().estados, [aparelho.id]: estado.ligado } })
    } catch (erro) {
      // O estado NÃO é atualizado no erro: mostrar "ligado" porque o botão foi clicado
      // seria mentir sobre uma lâmpada que continua apagada na sala.
      set({ erro: descrever(erro) })
    } finally {
      set({ comandando: null })
    }
  },

  importar: async (automatico = false) => {
    if (get().importando) return

    if (automatico) {
      // Sem credencial não há o que tentar, e um erro a cada 30 s de quem nunca
      // configurou a Tuya seria só barulho.
      const { tuyaClientId, tuyaClientSecret } = useSettingsStore.getState().settings
      if (!tuyaClientId.trim() || !tuyaClientSecret.trim()) return

      // Já falhou uma vez: a causa quase sempre é uma configuração no site da Tuya, que
      // não muda sozinha. Insistir a cada 30 s repetiria o mesmo erro na tela para
      // sempre — o botão manual continua ali para quando você tiver resolvido.
      if (get().importFalhou) return

      // Todo mundo já tem nome: não há o que buscar.
      if (get().aparelhos.every((aparelho) => aparelho.nome !== null)) return
    }

    set({ importando: true, erro: null })

    try {
      const trazidos = await importTuyaDevices(semente(get().aparelhos))

      // Os cartões ganham nome AGORA, sem pagar outra varredura de 10 s. A alternativa
      // era chamar `procurar` de novo só para o backend refazer a mesma junção.
      const porId = new Map(trazidos.map((aparelho) => [aparelho.id, aparelho]))
      set({
        aparelhos: get().aparelhos.map((aparelho) => {
          const trazido = porId.get(aparelho.id)
          if (!trazido) return aparelho

          return {
            ...aparelho,
            nome: trazido.nome.trim() || aparelho.nome,
            temChave: trazido.temChave,
          }
        }),
        importados: trazidos.length,
        importFalhou: false,
      })
    } catch (erro) {
      set({ erro: descrever(erro), importFalhou: true })
    } finally {
      set({ importando: false })
    }
  },
}))
