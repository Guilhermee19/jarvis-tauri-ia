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
   * O agente pedindo à UI algo que só ela sabe fazer — quem é dono do laço de preview
   * da câmera é o `sensorStore`, não o Rust. Emitido por `commands/chat.rs`.
   */
  UiAction: 'jarvis://ui-action',
} as const

/** Carga do {@link JarvisEvent.UiAction}. Espelha `AcaoDeUi::como_texto` no Rust. */
export type UiAction = 'webcam-on' | 'webcam-off'

type JarvisEventName = (typeof JarvisEvent)[keyof typeof JarvisEvent]

const NOOP_UNLISTEN: UnlistenFn = () => {}

export async function onJarvisEvent<T = void>(
  event: JarvisEventName,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return NOOP_UNLISTEN
  return listen<T>(event, (received) => handler(received.payload))
}
