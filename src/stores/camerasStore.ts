import { create } from 'zustand'
import {
  cameraSnapshot,
  cameraSubnets,
  listCameras,
  moveCamera,
  probeCamera,
  removeCamera,
  saveCamera,
  scanCameras,
  startCameras,
} from '@/lib/tauri'
import type { AlertaDeCamera } from '@/lib/tauri'
import type { Achado, Camera, Direcao, Sondagem } from '@/types'

/**
 * Quantos alertas ficam na tela.
 *
 * Curto de propósito: isto é "o que acabou de acontecer", não um histórico. O histórico
 * de verdade fica na memória do Jarvis, que é onde dá para perguntar depois.
 */
const ALERTAS_NA_TELA = 5

/**
 * As câmeras de segurança da casa, e o serviço que as traduz.
 *
 * Mora numa store e não no componente porque **subir o go2rtc leva segundos** — é um
 * processo nascendo. Fechar e reabrir a janela no meio disso jogaria fora uma espera já
 * paga, e é justo o que alguém faz quando acha que travou.
 *
 * Duas listas seriam um erro fácil aqui: o catálogo (o que está cadastrado) e o que está
 * no ar são a MESMA lista em momentos diferentes. Por isso `cameras` é uma só, e o que
 * distingue os dois momentos é o {@link CamerasState.baseUrl} estar preenchido.
 */
interface CamerasState {
  cameras: Camera[]
  /**
   * A base do go2rtc, ou `null` enquanto o serviço não estiver de pé.
   *
   * **É o sinal de "dá para mostrar vídeo"**, e não um booleano ao lado: a URL é
   * necessária para montar o player de qualquer jeito, e um booleano separado poderia
   * discordar dela. Mesma convenção de "vazio significa não se aplica" do Rust.
   */
  baseUrl: string | null
  ligando: boolean
  erro: string | null
  /** Qual câmera está em destaque. `null` = a grade com todas. */
  emFoco: string | null
  /** Quadros mais recentes por id, para a degradação graciosa e o cartão de preview. */
  quadros: Record<string, string>
  /** O que se mexeu recentemente, do mais novo para o mais velho. */
  alertas: AlertaDeCamera[]
  registrarAlerta: (alerta: AlertaDeCamera) => void
  limparAlertas: () => void

  /** Faixas sugeridas para varrer: a local, mais a de cada câmera já cadastrada. */
  prefixos: string[]
  /** O que a última varredura achou. `null` = ainda não varreu nesta sessão. */
  achados: Achado[] | null
  varrendo: boolean
  carregarPrefixos: () => Promise<void>
  varrer: (prefixo: string) => Promise<void>
  esquecerVarredura: () => void

  /** Carrega o catálogo sem encostar na rede. Instantâneo — é o que a janela chama ao abrir. */
  carregar: () => Promise<void>
  /** Sobe o go2rtc e guarda a base. Idempotente: já ligado, não faz nada. */
  ligar: () => Promise<void>
  focar: (id: string | null) => void
  atualizarQuadro: (id: string) => Promise<void>
  salvar: (camera: Camera) => Promise<void>
  remover: (id: string) => Promise<void>
  sondar: (host: string) => Promise<Sondagem>
  mover: (id: string, direcao: Direcao) => Promise<void>
}

export const useCamerasStore = create<CamerasState>((set, get) => ({
  cameras: [],
  baseUrl: null,
  ligando: false,
  erro: null,
  emFoco: null,
  quadros: {},
  alertas: [],

  registrarAlerta: (alerta) =>
    set((estado) => ({ alertas: [alerta, ...estado.alertas].slice(0, ALERTAS_NA_TELA) })),

  limparAlertas: () => set({ alertas: [] }),

  prefixos: [],
  achados: null,
  varrendo: false,

  carregarPrefixos: async () => {
    try {
      set({ prefixos: await cameraSubnets() })
    } catch {
      // Sem sugestão a tela ainda funciona — o campo fica em branco para digitar. Não
      // vale um erro na cara de quem só abriu o painel.
    }
  },

  varrer: async (prefixo) => {
    set({ varrendo: true, erro: null })
    try {
      set({ achados: await scanCameras(prefixo), varrendo: false })
    } catch (erro) {
      set({ erro: mensagem(erro), varrendo: false, achados: [] })
    }
  },

  esquecerVarredura: () => set({ achados: null }),

  carregar: async () => {
    try {
      set({ cameras: await listCameras() })
    } catch (erro) {
      set({ erro: mensagem(erro) })
    }
  },

  ligar: async () => {
    // Já de pé, ou já subindo: uma segunda chamada spawnaria nada (o Rust bate na porta
    // antes), mas deixaria dois `ligando` disputando o mesmo estado.
    if (get().baseUrl || get().ligando) return

    set({ ligando: true, erro: null })
    try {
      const { baseUrl, cameras } = await startCameras()
      set({ baseUrl, cameras, ligando: false })
    } catch (erro) {
      // O catálogo NÃO é limpo aqui: sem o go2rtc não há vídeo, mas a lista de câmeras
      // cadastradas continua sendo verdade — e é ela que permite consertar o cadastro
      // que talvez seja a causa.
      set({ erro: mensagem(erro), ligando: false })
    }
  },

  focar: (emFoco) => set({ emFoco }),

  atualizarQuadro: async (id) => {
    try {
      const quadro = await cameraSnapshot(id)
      set((estado) => ({ quadros: { ...estado.quadros, [id]: quadro } }))
    } catch {
      // Silencioso de propósito: isto roda em laço, e uma câmera fora do ar viraria uma
      // enxurrada de erros idênticos. Quem mostra o problema é o player, uma vez.
    }
  },

  salvar: async (camera) => {
    await saveCamera(camera)
    await get().carregar()

    // A configuração do go2rtc é derivada do catálogo, então uma câmera nova só existe
    // para ele depois de reescrever o arquivo — que é o que `startCameras` faz. Sem
    // isto, cadastrar e não ver imagem seria o comportamento normal.
    set({ baseUrl: null })
    await get().ligar()
  },

  remover: async (id) => {
    await removeCamera(id)
    set((estado) => {
      const quadros = { ...estado.quadros }
      delete quadros[id]
      return {
        quadros,
        emFoco: estado.emFoco === id ? null : estado.emFoco,
      }
    })
    await get().carregar()
  },

  sondar: (host) => probeCamera(host),

  mover: async (id, direcao) => {
    try {
      await moveCamera(id, direcao)
    } catch (erro) {
      set({ erro: mensagem(erro) })
    }
  },
}))

function mensagem(erro: unknown): string {
  return erro instanceof Error ? erro.message : String(erro)
}
