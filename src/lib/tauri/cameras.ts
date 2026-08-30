import type { Achado, Camera, CamerasLigadas, Direcao, Sondagem } from '@/types'
import { call } from './client'

/** Wrappers de `src-tauri/src/commands/cameras.rs`. */

/**
 * O que já se conhece, sem encostar na rede nem subir serviço — responde na hora.
 *
 * É o que a janela mostra no instante em que abre, antes de o go2rtc estar de pé.
 */
export function listCameras(): Promise<Camera[]> {
  return call<Camera[]>('list_cameras')
}

/**
 * Sobe o go2rtc (se ninguém atender) e devolve por onde falar com ele.
 *
 * **Demora na primeira vez de cada sessão** — é a subida de um processo. Depois disso
 * responde na hora, porque a chamada só bate na porta antes de spawnar qualquer coisa.
 * Quem chama precisa mostrar que está esperando.
 */
export function startCameras(): Promise<CamerasLigadas> {
  return call<CamerasLigadas>('start_cameras')
}

export function saveCamera(camera: Camera): Promise<void> {
  return call<void>('save_camera', { camera })
}

export function removeCamera(id: string): Promise<void> {
  return call<void>('remove_camera', { id })
}

/**
 * Um quadro da câmera, como `data:` URL.
 *
 * Mesmo formato do `captureWebcamFrame`, e é o que permite mostrar os dois numa `<img>`
 * pelo mesmo caminho. Serve de degradação graciosa quando o player de vídeo não carrega.
 */
export function cameraSnapshot(id: string): Promise<string> {
  return call<string>('camera_snapshot', { id })
}

/**
 * Pergunta a um endereço, por ONVIF, o que ele é e onde está o stream dele.
 *
 * Serve ao cadastro: em vez de descobrir a URL RTSP por tentativa, a câmera responde.
 * Falha em quem não fala ONVIF (o DVR não fala) — e aí o cadastro manual continua sendo
 * o caminho, não um erro.
 */
export function probeCamera(host: string): Promise<Sondagem> {
  return call<Sondagem>('probe_camera', { host })
}

/**
 * As faixas de rede que vale a pena varrer. Responde na hora.
 *
 * É a do próprio computador mais a de cada câmera já cadastrada — e a segunda parte
 * importa: numa casa com roteador em cascata, o PC fica numa faixa e as câmeras em outra,
 * então varrer só a local não acharia nada.
 */
export function cameraSubnets(): Promise<string[]> {
  return call<string[]>('camera_subnets')
}

/**
 * Varre a faixa (ex.: `"192.168.18"`) e devolve o que parecer câmera.
 *
 * **Leva alguns segundos e isso não é lentidão** — são centenas de sockets, a maioria
 * contra endereços vazios que só respondem no timeout. Quem chama precisa mostrar que
 * está esperando.
 */
export function scanCameras(prefixo: string): Promise<Achado[]> {
  return call<Achado[]>('scan_cameras', { prefixo })
}

/** Vira a câmera. Só as ONVIF têm este caminho; o DVR recusa com uma frase que explica. */
export function moveCamera(id: string, direcao: Direcao): Promise<void> {
  return call<void>('move_camera', { id, direcao })
}
