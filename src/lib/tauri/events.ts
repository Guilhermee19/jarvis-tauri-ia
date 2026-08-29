import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { isTauriRuntime } from './client'

/**
 * Eventos que o backend empurra para a UI (o caminho inverso do `invoke`).
 *
 * Evento é para o que a UI não pergunta, só desenha. O nível do microfone é o
 * primeiro caso real disso; a wake word e o streaming do agente entram aqui depois.
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
  /** A aba do navegador mudou de endereço sozinha — link, redirecionamento, rota de SPA. */
  BrowserUrl: 'jarvis://browser-url',
  /** A página pediu janela nova: clique do meio, `target="_blank"`, `window.open`. */
  BrowserNewTab: 'jarvis://browser-new-tab',
  /**
   * O agente pedindo à UI algo que só ela sabe fazer — quem é dono do laço de preview
   * da câmera é o `sensorStore`, não o Rust. Emitido por `commands/chat.rs`.
   */
  UiAction: 'jarvis://ui-action',
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
/** Carga do {@link JarvisEvent.BrowserUrl}. Espelha `MudouDeEndereco` no Rust. */
export interface MudouDeEndereco {
  id: string
  url: string
}

export type UiAction =
  | { tipo: 'webcam-on' }
  | { tipo: 'webcam-off' }
  | { tipo: 'tocando'; faixa: Faixa }
  | { tipo: 'abrir-site'; url: string }
  | { tipo: 'pesquisar'; query: string }

type JarvisEventName = (typeof JarvisEvent)[keyof typeof JarvisEvent]

const NOOP_UNLISTEN: UnlistenFn = () => {}

export async function onJarvisEvent<T = void>(
  event: JarvisEventName,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return NOOP_UNLISTEN
  return listen<T>(event, (received) => handler(received.payload))
}
