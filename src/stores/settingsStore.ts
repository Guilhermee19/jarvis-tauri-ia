import { create } from 'zustand'
import { getSettings, saveSettings } from '@/lib/tauri'
import { useSensorStore } from './sensorStore'
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

export const useSettingsStore = create<SettingsState>((set, get) => ({
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
    const anterior = get().settings
    set({ isSaving: true, error: null })
    try {
      await saveSettings(next)
      set({ settings: next, isSaving: false })

      // A resolução da webcam é negociada na abertura do stream, então salvá-la com
      // o preview rodando não muda nada até o próximo desligar/ligar. Reabrir aqui é
      // o que faz o ajuste valer na hora, em vez de o usuário concluir que quebrou.
      // O espelho NÃO entra: é CSS, já reage sozinho ao novo valor.
      if (
        next.webcamWidth !== anterior.webcamWidth ||
        next.webcamHeight !== anterior.webcamHeight
      ) {
        // Falhar ao reabrir vira `webcamError` lá dentro; salvar já deu certo, e
        // desfazer isso por causa da câmera seria pior.
        await useSensorStore.getState().reopenWebcam()
      }

      return true
    } catch (error) {
      set({ isSaving: false, error: describeError(error) })
      return false
    }
  },
}))
