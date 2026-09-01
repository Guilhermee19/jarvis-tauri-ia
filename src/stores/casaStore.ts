import { create } from 'zustand'
import {
  deviceState,
  discoverDevices,
  importTuyaDevices,
  irKeys,
  knownDevices,
  sendIrKey,
  sensorStates,
  setDeviceDp,
  setDeviceHidden,
  setDevicePower,
  setLight,
} from '@/lib/tauri'
import type { AjusteLuz, Aparelho, Controle, DetalheAparelho, Tecla } from '@/types'
import { useSettingsStore } from './settingsStore'

/** De quanto em quanto tempo a ronda repete, enquanto o painel estiver aberto. */
const RONDA_MS = 30_000

/**
 * De quanto em quanto tempo reler os sensores.
 *
 * Muito mais curto que a ronda porque as duas perguntam coisas de naturezas diferentes: a
 * ronda pergunta **quem existe** na rede, que muda quando alguém pluga um aparelho novo;
 * esta pergunta **o que está acontecendo**, e uma porta abre entre um piscar e outro.
 *
 * Cinco segundos é o meio-termo: uma conexão por gateway a cada cinco segundos é barato
 * numa rede local, e ninguém percebe cinco segundos de atraso numa porta que abriu.
 *
 * ponytail: o certo seria o gateway EMPURRAR a mudança — ele manda o aviso sozinho na
 * conexão aberta, e a latência cairia para o tempo do rádio. Isso pede uma thread viva no
 * Rust emitindo evento para a UI, e é outra tarefa.
 */
const SENSORES_MS = 5_000

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
  /** Liga ou desliga uma chave específica — a segunda tomada, a terceira tecla. */
  alternarChave: (aparelho: Aparelho, dp: string, ligado: boolean) => Promise<void>
  /** Tira da lista principal, ou devolve para ela. Só a tela muda. */
  ocultar: (aparelho: Aparelho, oculto: boolean) => Promise<void>
  /** Relê os sensores. Chamado pelo laço, não pela tela. */
  olharSensores: () => Promise<void>
  /**
   * As teclas de cada controle de infravermelho, por id.
   *
   * Buscadas na nuvem ao abrir os detalhes e guardadas depois disso: elas não mudam
   * sozinhas, e uma ida à internet por abertura de cartão seria desperdício.
   */
  controles: Record<string, Controle>
  carregarTeclas: (aparelho: Aparelho) => Promise<void>
  apertarTecla: (aparelho: Aparelho, tecla: Tecla) => Promise<void>
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
let vigia: ReturnType<typeof setInterval> | null = null

/**
 * As categorias que mudam de estado sozinhas.
 *
 * Espelha o `SENSORES` de `core/casa/controle.rs`, e a duplicação é consciente: lá ela
 * decide se um booleano é leitura ou botão; aqui, quem vale a pena reler sem parar.
 */
const SENSORES = new Set([
  'mcs',
  'mcs2',
  'pir',
  'hps',
  'ywbj',
  'rqbj',
  'sj',
  'wsdcg',
  'ldcg',
])

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
  controles: {},

  carregarTeclas: async (aparelho) => {
    if (get().controles[aparelho.id] !== undefined) return
    set({ detalhando: aparelho.id, erro: null })

    try {
      const controle = await irKeys(aparelho.emissor, aparelho.id)
      set({ controles: { ...get().controles, [aparelho.id]: controle } })
    } catch (erro) {
      set({ erro: descrever(erro) })
    } finally {
      set({ detalhando: null })
    }
  },

  apertarTecla: async (aparelho, tecla) => {
    const controle = get().controles[aparelho.id]
    if (controle === undefined || get().detalhando !== null) return

    set({ detalhando: aparelho.id, erro: null })

    try {
      await sendIrKey(aparelho.emissor, aparelho.id, controle.categoria, tecla)
    } catch (erro) {
      set({ erro: descrever(erro) })
    } finally {
      // Sem releitura: o emissor não sabe se a TV obedeceu — ele pisca um LED e pronto.
      // Fingir uma confirmação aqui seria inventar informação que não existe.
      set({ detalhando: null })
    }
  },

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

  alternarChave: async (aparelho, dp, ligado) => {
    if (get().detalhando !== null) return
    set({ detalhando: aparelho.id, erro: null })

    try {
      const detalhe = await setDeviceDp(aparelho.id, aparelho.ip, aparelho.versao, dp, ligado)
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
    vigia = setInterval(() => void get().olharSensores(), SENSORES_MS)
  },

  pararRonda: () => {
    if (ronda !== null) clearInterval(ronda)
    if (vigia !== null) clearInterval(vigia)
    ronda = null
    vigia = null
  },

  olharSensores: async () => {
    // Categoria e não capacidade: perguntar A TODOS os aparelhos de cinco em cinco
    // segundos acenderia o rádio de tudo na casa para saber o que já se sabe. Sensor é o
    // que muda sozinho; o resto muda quando alguém manda.
    const ids = get()
      .aparelhos.filter((aparelho) => SENSORES.has(aparelho.categoria) && aparelho.temChave)
      .map((aparelho) => aparelho.id)

    if (ids.length === 0 || get().detalhando !== null) return

    try {
      const lidos = await sensorStates(ids)
      if (lidos.length === 0) return

      set({ detalhes: { ...get().detalhes, ...Object.fromEntries(lidos) } })
    } catch {
      // Silêncio: é um laço de fundo. Um erro aqui viraria a mesma mensagem na tela a
      // cada cinco segundos, e o que ele diria já está no cartão — o sensor sem leitura.
    }
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

      // Todo mundo já tem nome: não há o que buscar. Lista vazia não conta: aí a nuvem é
      // a única fonte que sobrou, e é justamente quando mais precisa rodar.
      const conhecidos = get().aparelhos
      if (conhecidos.length > 0 && conhecidos.every((aparelho) => aparelho.nome !== null)) return
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
