import { create } from 'zustand'
import {
  captureWebcamFrame,
  closeWebcam,
  isRecording,
  openWebcam,
  startRecording,
  stopRecording,
  transcribe,
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
  /**
   * Liga ou desliga explicitamente. Existe para o agente ("abre a webcam") passar
   * pelo MESMO caminho do botão — alternar às cegas desligaria a câmera se ela já
   * estivesse ligada, que é o oposto do pedido.
   */
  setWebcam: (on: boolean) => Promise<void>

  isMicOn: boolean
  isMicBusy: boolean
  /** Pico de 0 a 1 do último intervalo, vindo do evento `jarvis://mic-level`. */
  micLevel: number
  micError: string | null
  lastRecording: Recording | null
  toggleMic: () => Promise<void>
  setMicLevel: (level: number) => void

  /**
   * Ditado do chat (segurar o botão para falar). Mora AQUI, junto do `toggleMic`,
   * porque o dono do gravador tem que ser um só: se o botão do chat chamasse
   * `startRecording` por conta própria com o microfone da bancada ligado, o backend
   * responderia "já existe uma gravação em andamento".
   */
  isDictating: boolean
  isTranscribing: boolean
  /**
   * Erro do ditado, SEPARADO do `micError` da bancada.
   *
   * Separado porque os dois têm plateias diferentes: o `micError` aparece no HUD da
   * home, que fica atrás do painel de chat — quem clicou "Falar" nunca ia ver. E era
   * exatamente esse o bug: a falha ia para um alerta escondido e o botão parecia não
   * fazer nada. Erro de ditado tem que aparecer ao lado do botão que o causou.
   */
  dictationError: string | null
  startDictation: () => Promise<void>
  /** Devolve o transcrito, ou string vazia se nada foi ouvido ou algo falhou. */
  stopDictation: () => Promise<string>
  clearDictationError: () => void
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

    toggleWebcam: async () => get().setWebcam(!get().isWebcamOn),

    setWebcam: async (on: boolean) => {
      if (get().isWebcamBusy || get().isWebcamOn === on) return
      set({ isWebcamBusy: true, webcamError: null })

      try {
        if (on) {
          await openWebcam()
          set({ isWebcamOn: true })
          runPreviewLoop()
        } else {
          // Desliga antes de fechar: o laço checa esta flag e para sozinho.
          stopPreviewLoop()
          set({ isWebcamOn: false, webcamFrame: null })
          await closeWebcam()
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

    isDictating: false,
    isTranscribing: false,
    dictationError: null,

    clearDictationError: () => set({ dictationError: null }),

    startDictation: async () => {
      const { isMicOn, isMicBusy, isDictating } = get()
      if (isDictating || isMicBusy) return
      // Recusar continua certo — o dono do gravador tem que ser um só —, mas AGORA
      // com motivo na tela. Recusar em silêncio era indistinguível de um botão morto.
      if (isMicOn) {
        set({
          dictationError:
            'o microfone está ocupado pela bancada de diagnóstico — pare a gravação de lá primeiro',
        })
        return
      }

      set({ isMicBusy: true, dictationError: null })
      try {
        await startRecording()
        set({ isDictating: true })
      } catch (cause) {
        // Uma gravação órfã no backend — recarregar a UI no meio de um ditado, o que
        // o hot reload do `tauri dev` faz o tempo todo — deixava o botão inutilizável
        // para sempre: toda tentativa batia em "já existe uma gravação em andamento"
        // e não havia caminho de volta pela interface. Descarta a órfã e tenta UMA
        // vez; um segundo fracasso é problema de verdade e vai para a tela.
        if (await descartarGravacaoOrfa()) {
          try {
            await startRecording()
            set({ isDictating: true })
            return
          } catch (segunda) {
            set({ dictationError: describe(segunda) })
            return
          }
        }
        set({ dictationError: describe(cause) })
      } finally {
        set({ isMicBusy: false })
      }
    },

    stopDictation: async () => {
      if (!get().isDictating) return ''
      set({ isDictating: false, isTranscribing: true, micLevel: 0 })

      try {
        set({ lastRecording: await stopRecording() })
        return await transcribe()
      } catch (cause) {
        // Erro vira aviso e string vazia: o botão de falar não pode deixar o chat
        // num estado travado só porque o Whisper não estava lá.
        set({ dictationError: describe(cause) })
        return ''
      } finally {
        set({ isTranscribing: false })
      }
    },
  }
})

/**
 * Fecha uma gravação que ficou aberta no backend sem a UI saber, e diz se havia uma.
 *
 * O WAV que ela deixa é lixo — ninguém pediu — mas `stop_recording` é o único jeito
 * de soltar o dispositivo: o `Recorder` é consumido no `stop`, e não existe comando
 * de "cancelar".
 */
async function descartarGravacaoOrfa(): Promise<boolean> {
  try {
    if (!(await isRecording())) return false
    await stopRecording()
    return true
  } catch {
    // Se nem dá para perguntar ao backend, não há recuperação a tentar — o erro
    // original é o que interessa para o usuário.
    return false
  }
}
