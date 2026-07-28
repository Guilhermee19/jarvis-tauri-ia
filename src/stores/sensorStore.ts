import { create } from 'zustand'
import {
  captureWebcamFrame,
  closeWebcam,
  openWebcam,
  startRecording,
  stopRecording,
} from '@/lib/tauri'
import type { Recording } from '@/types'

/**
 * Alvo de ~25 quadros por segundo. É um PERÍODO, não uma pausa: o backend guarda
 * sempre o quadro mais recente, então o custo daqui é só a viagem pelo IPC, e
 * descontá-la do intervalo é o que mantém a cadência estável em vez de somar
 * "trabalho + pausa" e entregar um fps diferente em cada máquina.
 */
const TARGET_FRAME_MS = 40

/**
 * Estado dos sensores que ficam ligados por conta do usuário — não de uma tela.
 *
 * Mora aqui, e não nos componentes, porque webcam e microfone agora têm dois
 * consumidores: os botões da barra de ícones e o fundo da home, mais a bancada de
 * diagnóstico. Dois laços de captura chamando `capture_webcam_frame` disputariam a
 * mesma câmera e cada um receberia metade dos quadros; com o laço aqui, existe um só.
 */
interface SensorState {
  isWebcamOn: boolean
  isWebcamBusy: boolean
  /** `data:` URL do último quadro, ou `null` com a câmera desligada. */
  webcamFrame: string | null
  webcamError: string | null
  toggleWebcam: () => Promise<void>

  isMicOn: boolean
  isMicBusy: boolean
  /** Pico de 0 a 1 do último intervalo, vindo do evento `jarvis://mic-level`. */
  micLevel: number
  micError: string | null
  lastRecording: Recording | null
  toggleMic: () => Promise<void>
  setMicLevel: (level: number) => void
}

function describe(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

/**
 * O handle do laço fica fora do estado de propósito: nada aqui é desenhado, e
 * colocá-lo no store faria cada quadro invalidar um valor que ninguém lê.
 */
let previewTimer: ReturnType<typeof setTimeout> | null = null

function stopPreviewLoop() {
  if (previewTimer !== null) clearTimeout(previewTimer)
  previewTimer = null
}

export const useSensorStore = create<SensorState>((set, get) => {
  /**
   * `setTimeout` encadeado em vez de `setInterval`: se um quadro demorar mais que o
   * intervalo, os pedidos se empilhariam e a câmera nunca alcançaria a fila.
   */
  function runPreviewLoop() {
    async function tick() {
      if (!get().isWebcamOn) return
      const startedAt = performance.now()

      try {
        const frame = await captureWebcamFrame()
        if (!get().isWebcamOn) return
        set({ webcamFrame: frame.dataUrl })
      } catch (cause) {
        // Falhar no meio do preview (câmera arrancada da USB) desliga tudo em vez
        // de repetir a mesma falha 25 vezes por segundo.
        stopPreviewLoop()
        set({ isWebcamOn: false, webcamFrame: null, webcamError: describe(cause) })
        return
      }

      const remaining = TARGET_FRAME_MS - (performance.now() - startedAt)
      previewTimer = setTimeout(() => void tick(), Math.max(0, remaining))
    }

    void tick()
  }

  return {
    isWebcamOn: false,
    isWebcamBusy: false,
    webcamFrame: null,
    webcamError: null,

    toggleWebcam: async () => {
      if (get().isWebcamBusy) return
      set({ isWebcamBusy: true, webcamError: null })

      try {
        if (get().isWebcamOn) {
          // Desliga antes de fechar: o laço checa esta flag e para sozinho.
          stopPreviewLoop()
          set({ isWebcamOn: false, webcamFrame: null })
          await closeWebcam()
        } else {
          await openWebcam()
          set({ isWebcamOn: true })
          runPreviewLoop()
        }
      } catch (cause) {
        stopPreviewLoop()
        set({ isWebcamOn: false, webcamFrame: null, webcamError: describe(cause) })
      } finally {
        set({ isWebcamBusy: false })
      }
    },

    isMicOn: false,
    isMicBusy: false,
    micLevel: 0,
    micError: null,
    lastRecording: null,

    toggleMic: async () => {
      if (get().isMicBusy) return
      set({ isMicBusy: true, micError: null })

      try {
        if (get().isMicOn) {
          const recording = await stopRecording()
          set({ isMicOn: false, micLevel: 0, lastRecording: recording })
        } else {
          await startRecording()
          set({ isMicOn: true })
        }
      } catch (cause) {
        set({ isMicOn: false, micLevel: 0, micError: describe(cause) })
      } finally {
        set({ isMicBusy: false })
      }
    },

    setMicLevel: (level) => set({ micLevel: level }),
  }
})
