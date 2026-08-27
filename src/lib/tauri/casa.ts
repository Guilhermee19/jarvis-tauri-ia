import type { Varredura } from '@/types'
import { call } from './client'

/** Wrappers de `src-tauri/src/commands/casa.rs`. */

/**
 * Escuta a rede à procura de aparelhos Tuya (Positivo, EKAZA e companhia).
 *
 * **Demora alguns segundos e isso não é lentidão.** Os aparelhos se anunciam sozinhos
 * de tempos em tempos, e não há como pedir que falem antes da hora — a chamada é uma
 * janela de escuta, não uma consulta. Quem chama precisa mostrar que está esperando.
 */
export function discoverDevices(): Promise<Varredura> {
  return call<Varredura>('discover_devices')
}
