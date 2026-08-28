import { call } from './client'
import type { Faixa } from './events'
import type { Metricas } from '@/types'

/** Wrappers de `src-tauri/src/commands/system.rs`. */

export function showWindow(): Promise<void> {
  return call<void>('show_window')
}

export function hideWindow(): Promise<void> {
  return call<void>('hide_window')
}

export function toggleWindow(): Promise<void> {
  return call<void>('toggle_window')
}

export function minimizeWindow(): Promise<void> {
  return call<void>('minimize_window')
}

/** Alterna e devolve o estado NOVO — o botão troca de ícone sem uma segunda viagem. */
export function toggleMaximizeWindow(): Promise<boolean> {
  return call<boolean>('toggle_maximize_window')
}

/** Para ressincronizar quando o usuário maximiza por fora (Win+↑, duplo clique). */
export function isWindowMaximized(): Promise<boolean> {
  return call<boolean>('is_window_maximized')
}

/**
 * O que dá para saber do player sem OAuth: o título da janela do Spotify.
 *
 * `"Artista - Música"` enquanto toca, e nada quando pausa — é esse par que deixa o
 * widget congelar a barra de progresso em vez de continuar contando e mentir.
 */
export interface NowPlaying {
  titulo: string | null
  tocando: boolean
}

export function nowPlaying(): Promise<NowPlaying> {
  return call<NowPlaying>('now_playing')
}

/**
 * Capa e duração pelo TÍTULO da janela do Spotify. `null` quando não há credencial ou
 * a busca não acha — aí o widget cai no texto do título, que já é melhor que nada.
 *
 * Só chame quando o título MUDAR: perguntar a cada sincronia gastaria a cota da API
 * para receber sempre a mesma resposta.
 */
export function identifyTrack(title: string): Promise<Faixa | null> {
  return call<Faixa | null>('identify_track', { title })
}

export type MediaKey = 'play-pause' | 'next' | 'previous'

/** Tecla de mídia global: quem recebe é o player em foco, seja qual for. */
export function pressMediaKey(key: MediaKey): Promise<void> {
  return call<void>('press_media_key', { key })
}

export function quitApp(): Promise<void> {
  return call<void>('quit_app')
}

/**
 * Quanto do computador está sendo usado agora.
 *
 * A PRIMEIRA chamada abre os contadores do Windows e sai zerada: uso de processador é
 * uma taxa entre duas leituras, e a primeira não tem de quando medir.
 */
export function performanceMetrics(): Promise<Metricas> {
  return call<Metricas>('performance_metrics')
}
