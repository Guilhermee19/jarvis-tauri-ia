import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { isTauriRuntime } from './client'

/**
 * Eventos que o backend empurra para a UI (o caminho inverso do `invoke`).
 *
 * Hoje só a bandeja usa. As features de voz vão publicar aqui também
 * (`jarvis://wake-word`, `jarvis://transcript`), e o streaming do agente idem.
 */
export const JarvisEvent = {
  /** Emitido pelo item "Configurações" do menu da bandeja (`src-tauri/src/tray.rs`). */
  OpenSettings: 'jarvis://open-settings',
} as const

const NOOP_UNLISTEN: UnlistenFn = () => {}

export async function onJarvisEvent(
  event: (typeof JarvisEvent)[keyof typeof JarvisEvent],
  handler: () => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return NOOP_UNLISTEN
  return listen(event, () => handler())
}
