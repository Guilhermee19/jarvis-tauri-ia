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

export function quitApp(): Promise<void> {
  return call<void>('quit_app')
}
