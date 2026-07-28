import type { AppSettings } from '@/types'
import { call } from './client'

/** Wrappers de `src-tauri/src/commands/settings.rs`. */

export function getSettings(): Promise<AppSettings> {
  return call<AppSettings>('get_settings')
}

export function saveSettings(settings: AppSettings): Promise<void> {
  return call<void>('save_settings', { settings })
}
