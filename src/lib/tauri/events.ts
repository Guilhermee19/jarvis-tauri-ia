import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { isTauriRuntime } from './client'
import type { Cotacao } from '@/types/cotacoes'

/**
 * Eventos que o backend empurra para a UI (o caminho inverso do `invoke`).
 *
 * Evento é para o que a UI não pergunta, só desenha. O nível do microfone foi o
 * primeiro caso real disso, e o streaming da resposta ({@link JarvisEvent.ReplyChunk})
 * é o mais recente; a wake word entra aqui depois.
 */
export const JarvisEvent = {
  /** Emitido pelo item "Configurações" do menu da bandeja (`src-tauri/src/tray.rs`). */
  OpenSettings: 'jarvis://open-settings',
  /** Pico do microfone (0–1), ~20×/s enquanto há gravação em andamento. */
  MicLevel: 'jarvis://mic-level',
  /**
   * O mesmo, para o áudio que SAI — ~20×/s enquanto o Jarvis fala, e um zero no fim.
   *
   * Irmão do de cima e não um evento só com um campo "fonte": os dois têm ciclos de vida
   * independentes, e juntá-los obrigaria todo consumidor a filtrar algo que o nome já
   * diz.
   */
  TtsLevel: 'jarvis://tts-level',
  /**
   * Uma frase da resposta, assim que ela fica pronta — e antes de o modelo escrever o
   * resto.
   *
   * É o par escrito da fala: a mesma frase vai para a caixa de som e para a bolha no
   * mesmo instante. Precisa ser evento porque o retorno do `send_message` é um só e chega
   * no fim, quando a resposta já foi inteiramente ouvida.
   */
  ReplyChunk: 'jarvis://reply-chunk',
  /**
   * As notas do Jarvis mudaram — o grafo do conhecimento que estiver aberto se redesenha.
   *
   * Sem carga: quem recebe relê o grafo inteiro. Só chega quando mudou de verdade (uma
   * busca que virou nota, a nota da conversa, um "esquece X"), então reagir a ele não
   * custa uma releitura por mensagem.
   */
  MemoriaMudou: 'jarvis://memoria-mudou',
  /** A aba do navegador mudou de endereço sozinha — link, redirecionamento, rota de SPA. */
  BrowserUrl: 'jarvis://browser-url',
  /** A página pediu janela nova: clique do meio, `target="_blank"`, `window.open`. */
  BrowserNewTab: 'jarvis://browser-new-tab',
  /**
   * O agente pedindo à UI algo que só ela sabe fazer — quem é dono do laço de preview
   * da câmera é o `sensorStore`, não o Rust. Emitido por `commands/chat.rs`.
   */
  UiAction: 'jarvis://ui-action',
  /**
   * Movimento numa câmera vigiada, já filtrado pelo modelo de visão.
   *
   * Chega raramente por construção: o Rust só acorda o modelo quando a diferença entre
   * dois quadros estoura o limiar, e só emite quando ele confirma que havia gente,
   * animal ou veículo. Emitido por `commands/cameras.rs`.
   */
  CameraAlert: 'jarvis://camera-alert',
} as const

/** Uma faixa do Spotify. Espelha `core::music::Faixa`. */
export interface Faixa {
  id: string
  titulo: string
  artista: string
  /** URL da arte do álbum, ou `null` em faixa sem capa cadastrada. */
  capa: string | null
  duracaoMs: number
}

/**
 * Carga do {@link JarvisEvent.UiAction}. Espelha o enum `AcaoDeUi` do Rust, que é
 * serializado com tag externa — por isso o `tipo` discrimina e só uma das variantes
 * carrega dados.
 */
/** Carga do {@link JarvisEvent.CameraAlert}. Espelha `commands::cameras::Alerta`. */
export interface AlertaDeCamera {
  /** O id, para saber qual cartão destacar. */
  camera: string
  /** O nome falado — é o que a pessoa lê. */
  nome: string
  /** O que o modelo disse ter visto. */
  resposta: string
  quando: number
}

/**
 * Carga do {@link JarvisEvent.ReplyChunk}. Espelha `Pedaco` em `commands/chat.rs`.
 *
 * O `turno` é o crachá que a tela mandou no `sendMessage`: frase de outro turno é de uma
 * resposta que foi interrompida, e não pertence à bolha que está sendo escrita.
 */
export interface PedacoDaResposta {
  turno: string
  frase: string
}

/** Carga do {@link JarvisEvent.BrowserUrl}. Espelha `MudouDeEndereco` no Rust. */
export interface MudouDeEndereco {
  id: string
  url: string
}

export type UiAction =
  | { tipo: 'webcam-on' }
  | { tipo: 'webcam-off' }
  /** `camera` é o **id** da câmera, já resolvido pelo catálogo no Rust — não o nome falado. */
  | { tipo: 'camera-on'; camera: string }
  | { tipo: 'camera-off' }
  /** Guarda o rosto de quem está na webcam agora sob este nome ("eu sou o Guilherme"). */
  | { tipo: 'cadastrar-rosto'; nome: string }
  | { tipo: 'tocando'; faixa: Faixa }
  | { tipo: 'abrir-site'; url: string }
  | { tipo: 'pesquisar'; query: string }
  /** Abre o card de cotações com os números que o agente ACABOU de buscar. */
  | { tipo: 'cotacoes'; cotacoes: Cotacao[] }

type JarvisEventName = (typeof JarvisEvent)[keyof typeof JarvisEvent]

const NOOP_UNLISTEN: UnlistenFn = () => {}

export async function onJarvisEvent<T = void>(
  event: JarvisEventName,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return NOOP_UNLISTEN
  return listen<T>(event, (received) => handler(received.payload))
}
