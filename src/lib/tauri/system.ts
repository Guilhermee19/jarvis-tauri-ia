import { call } from './client'

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

export function quitApp(): Promise<void> {
  return call<void>('quit_app')
}
