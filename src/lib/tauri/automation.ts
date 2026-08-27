import type { CapturedImage, MonitorInfo, WebcamResolution } from '@/types'
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
 *
 * `maxWidth` limita a imagem DEVOLVIDA, não a captura: a câmera continua no que
 * estiver configurado. É o que torna 1080p usável — a prévia pede o tamanho da
 * janela e o quadro deixa de atravessar o IPC com quase dez vezes mais bytes do que
 * a tela consegue mostrar. Omitir devolve o quadro inteiro, que é o que o modelo quer.
 */
export function captureWebcamFrame(maxWidth?: number): Promise<CapturedImage> {
  // `null` explícito, como no `captureScreenshot`: o `Option<u32>` do Rust espera a
  // chave nula, não a chave ausente.
  return call<CapturedImage>('capture_webcam_frame', { maxWidth: maxWidth ?? null })
}

/**
 * Resoluções que a câmera aceita, da maior para a menor.
 *
 * Abre o dispositivo só para perguntar, então NÃO chame com o preview rodando se der
 * para evitar — são dois pedidos à mesma câmera. As configurações consultam uma vez,
 * ao abrir a tela.
 */
export function listWebcamResolutions(): Promise<WebcamResolution[]> {
  return call<WebcamResolution[]>('list_webcam_resolutions')
}

export function listMonitors(): Promise<MonitorInfo[]> {
  return call<MonitorInfo[]>('list_monitors')
}

/**
 * Sem `monitorId`, captura o monitor principal.
 *
 * A bancada não passa `maxWidth`: ela existe para PROVAR que a captura saiu certa, e
 * uma imagem reduzida esconderia justamente o defeito que se quer ver. Reduzir é
 * assunto de quem manda a tela para um modelo, e isso acontece dentro do agente.
 */
export function captureScreenshot(monitorId?: number, maxWidth?: number): Promise<CapturedImage> {
  // `null` explícito em vez de `undefined`: chave ausente e chave nula não são a
  // mesma coisa do outro lado, e o `Option<u32>` do Rust espera a nula.
  return call<CapturedImage>('capture_screenshot', {
    monitorId: monitorId ?? null,
    maxWidth: maxWidth ?? null,
  })
}
