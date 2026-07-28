import { create } from 'zustand'
import { getSettings, saveSettings } from '@/lib/tauri'
import { DEFAULT_SETTINGS, type AppSettings } from '@/types'

interface SettingsState {
  settings: AppSettings
  isLoaded: boolean
  isSaving: boolean
  error: string | null
  load: () => Promise<void>
  save: (next: AppSettings) => Promise<boolean>
}

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export const useSettingsStore = create<SettingsState>((set) => ({
  settings: DEFAULT_SETTINGS,
  isLoaded: false,
  isSaving: false,
  error: null,

  load: async () => {
    try {
      set({ settings: await getSettings(), isLoaded: true, error: null })
    } catch (error) {
      // Sem backend a UI ainda funciona com os defaults, então isso não é fatal.
      set({ isLoaded: true, error: describeError(error) })
    }
  },

  save: async (next: AppSettings) => {
    set({ isSaving: true, error: null })
    try {
      await saveSettings(next)
      set({ settings: next, isSaving: false })
      return true
    } catch (error) {
      set({ isSaving: false, error: describeError(error) })
      return false
    }
  },
}))
