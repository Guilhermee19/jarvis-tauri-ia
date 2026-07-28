import type { CapturedImage, MonitorInfo } from '@/types'
import { call } from './client'

/** Wrappers de `src-tauri/src/commands/automation.rs`. */

/**
 * Mantém a câmera aberta entre capturas. Abrir custa centenas de milissegundos,
 * então o preview ao vivo depende de a sessão continuar de pé.
 */
export function openWebcam(): Promise<void> {
  return call<void>('open_webcam')
}

export function closeWebcam(): Promise<void> {
  return call<void>('close_webcam')
}

export function isWebcamOpen(): Promise<boolean> {
  return call<boolean>('is_webcam_open')
}

/**
 * Frame atual. Com a webcam fechada, abre e fecha em volta da captura — é isso que
 * permite a v0.2 tirar uma foto pontual chamando exatamente esta função.
 */
export function captureWebcamFrame(): Promise<CapturedImage> {
  return call<CapturedImage>('capture_webcam_frame')
}

export function listMonitors(): Promise<MonitorInfo[]> {
  return call<MonitorInfo[]>('list_monitors')
}

/** Sem `monitorId`, captura o monitor principal. */
export function captureScreenshot(monitorId?: number): Promise<CapturedImage> {
  // `null` explícito em vez de `undefined`: chave ausente e chave nula não são a
  // mesma coisa do outro lado, e o `Option<u32>` do Rust espera a nula.
  return call<CapturedImage>('capture_screenshot', { monitorId: monitorId ?? null })
}
