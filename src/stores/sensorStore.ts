import { create } from 'zustand'
import {
  captureWebcamFrame,
  closeWebcam,
  isRecording,
  openWebcam,
  startRecording,
  stopRecording,
  stopSpeaking,
  transcribe,
} from '@/lib/tauri'
import { avaliarTurno, iniciarTurno, type DecisaoVad, type TurnoVad } from '@/lib/vad'
import { useChatStore } from './chatStore'
import { useSettingsStore } from './settingsStore'
import type { Recording } from '@/types'

/**
 * Alvo de ~25 quadros por segundo. É um PERÍODO, não uma pausa: o backend guarda
 * sempre o quadro mais recente, então o custo daqui é só a viagem pelo IPC, e
 * descontá-la do intervalo é o que mantém a cadência estável em vez de somar
 * "trabalho + pausa" e entregar um fps diferente em cada máquina.
 */
const TARGET_FRAME_MS = 40

/**
 * De quanto em quanto tempo o modo conversa olha o medidor do microfone.
 *
 * É um `setInterval` lendo `micLevel`, e não uma reação à mudança do valor, porque
 * em silêncio o pico chega 0 repetido — e o zustand não notifica quem seleciona um
 * valor igual ao anterior. Reagir à mudança perderia exatamente o caso que importa,
 * que é justamente o silêncio parado.
 *
 * 200 ms amostra um a cada quatro eventos do backend (que vêm a 20 Hz). É granular o
 * bastante para medir 1,2 s de silêncio e não faz o laço girar à toa.
 */
const AMOSTRAGEM_VAD_MS = 200

/**
 * Teto de largura do quadro da PRÉVIA, em pixels de dispositivo.
 *
 * A janela tem ~620px de largura; pedir 1920 para desenhar em 620 significa mandar
 * ~9× os pixels que cabem na tela — e o custo aparece inteiro, porque cada quadro
 * vira base64 (+33%), atravessa o IPC como string JSON e é decodificado de novo pelo
 * webview, 25 vezes por segundo. Era isso que travava a prévia em 1080p.
 *
 * Multiplica pelo `devicePixelRatio` porque em tela HiDPI 620 CSS px são 1240 px
 * reais, e pedir 620 ali deixaria a imagem BORRADA — que é o defeito oposto.
 *
 * O teto de 1920 impede que um monitor 4K peça mais que a própria câmera entrega.
 */
/**
 * Quantos pixels de largura pedir por quadro.
 *
 * O teto é 1280 e não 1920 por causa de um penhasco: numa tela de alta densidade, uma
 * janela de 620 px pediria 1240 — e se a câmera estiver em 1080p, um pedido de 1920
 * cairia no caso "já cabe", que **não reduz nada**. Aí o quadro atravessa o IPC inteiro,
 * ~530 KB de base64 25 vezes por segundo, e a prévia trava sem que nenhuma conta de
 * redimensionamento apareça no perfil.
 *
 * Recalculado a cada quadro: a janela é redimensionável, e arrastar a borda tem que mudar
 * o tamanho pedido sem reabrir a câmera.
 */
function larguraDaPrevia(): number {
  if (typeof window === 'undefined') return 1280

  const densidade = window.devicePixelRatio || 1
  return Math.min(1280, Math.max(640, Math.round(window.innerWidth * densidade)))
}

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
  /**
   * Fecha e reabre, para a câmera renegociar o formato.
   *
   * Existe porque a resolução é escolhida na ABERTURA do stream: salvar 1080p com o
   * preview rodando não mudaria nada até o próximo desligar/ligar, e o ajuste
   * pareceria simplesmente não funcionar. Com a webcam desligada é no-op — não é
   * papel de salvar configuração ligar câmera.
   */
  reopenWebcam: () => Promise<void>

  isMicOn: boolean
  isMicBusy: boolean
  /** Pico de 0 a 1 do último intervalo, vindo do evento `jarvis://mic-level`. */
  micLevel: number
  /**
   * O mesmo, para o áudio que o Jarvis está FALANDO.
   *
   * Mora aqui ao lado do microfone, e não no `chatStore` junto do `isSpeaking`, porque a
   * divisão é entre naturezas e não entre features: os dois são medidas do encanamento de
   * áudio, com a mesma faixa e a mesma cadência. O `isSpeaking` continua lá porque ele é
   * uma FASE da resposta — ele começa antes do som existir, enquanto a ElevenLabs ainda
   * está sintetizando.
   */
  ttsLevel: number
  micError: string | null
  lastRecording: Recording | null
  toggleMic: () => Promise<void>
  setMicLevel: (level: number) => void
  setTtsLevel: (level: number) => void

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

  /**
   * Modo conversa: o microfone fica aberto, o SILÊNCIO marca o fim da sua frase e a
   * resposta volta falada — sem clique nenhum entre uma frase e a próxima.
   *
   * Mora aqui pelo mesmo motivo do ditado: o gravador tem um dono só. E o laço fica
   * na store, e não num hook, para sobreviver a fechar o painel de chat — desligar a
   * janelinha não é dizer "pare de me ouvir".
   */
  isConversing: boolean
  toggleConversation: () => Promise<void>
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

/** Mesma razão do `previewTimer`: é maquinário do laço, não estado desenhado. */
let conversationTimer: ReturnType<typeof setInterval> | null = null

function stopConversationLoop() {
  if (conversationTimer !== null) clearInterval(conversationTimer)
  conversationTimer = null
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
        // Recalculado a cada quadro de propósito: a janela é redimensionável, e
        // arrastar a borda tem que mudar o tamanho pedido sem reabrir a câmera.
        const frame = await captureWebcamFrame(larguraDaPrevia())
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

  /**
   * O laço do modo conversa: amostra o medidor, e quando o VAD diz que a frase
   * acabou, encadeia transcrever → responder → falar → voltar a ouvir.
   *
   * `ocupado` existe porque o turno leva SEGUNDOS (Whisper + Ollama + ElevenLabs) e
   * o timer continua disparando durante todos eles. Sem a trava, a amostra seguinte
   * tentaria fechar um turno que já está sendo fechado.
   */
  function runConversationLoop() {
    let turno: TurnoVad = iniciarTurno(Date.now())
    let ocupado = false

    conversationTimer = setInterval(() => {
      if (ocupado || !get().isConversing) return

      const passo = avaliarTurno(turno, get().micLevel, Date.now())
      turno = passo.turno
      if (passo.decisao === 'ouvindo') return

      ocupado = true
      void fecharTurno(passo.decisao).finally(() => {
        turno = iniciarTurno(Date.now())
        ocupado = false
      })
    }, AMOSTRAGEM_VAD_MS)

    async function fecharTurno(decisao: DecisaoVad) {
      if (decisao === 'reciclar') {
        // Nada foi dito, então não há o que transcrever: pagar os segundos do
        // Whisper para ele confirmar que ouviu silêncio seria desperdício puro.
        set({ isDictating: false, micLevel: 0 })
        await stopRecording().catch(() => undefined)
      } else {
        const ouvido = await get().stopDictation()
        // Desligar o modo no meio de um turno descarta a frase de propósito: o
        // clique foi "pare", e mandar a última coisa ouvida seria o contrário.
        if (!get().isConversing) return

        // O modo conversa manda TUDO direto, sem exigir o vocativo que o botão de
        // ditado exige (`comandoEnderecado`): ligar o modo já é a declaração de que
        // a fala é para ele. O `send` responde E fala — só volta quando ele calou,
        // que é exatamente quando o microfone pode reabrir sem ouvir a si mesmo.
        if (ouvido) await useChatStore.getState().send(ouvido)
      }

      if (!get().isConversing) return
      await get().startDictation()

      // Não conseguiu reabrir o microfone (dispositivo arrancado, permissão
      // revogada): desliga o modo em vez de girar para sempre sem gravar nada. O
      // `startDictation` já deixou o motivo em `dictationError`.
      if (!get().isDictating) {
        stopConversationLoop()
        set({ isConversing: false })
      }
    }
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

    reopenWebcam: async () => {
      if (!get().isWebcamOn) return
      await get().setWebcam(false)
      await get().setWebcam(true)
    },

    isMicOn: false,
    isMicBusy: false,
    micLevel: 0,
    ttsLevel: 0,
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
    setTtsLevel: (level) => set({ ttsLevel: level }),

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

    isConversing: false,

    toggleConversation: async () => {
      if (get().isConversing) {
        stopConversationLoop()
        set({ isConversing: false })
        // Calar vem ANTES de soltar o microfone: quem clicou em desligar quer
        // silêncio agora, não quando a frase em curso terminar.
        await stopSpeaking().catch(() => undefined)
        await get().stopDictation()
        return
      }

      // Sem voz configurada o modo seria só o ditado automático — ele ouviria, e
      // responderia por escrito, calado. Recusar na hora do clique diz onde
      // resolver; deixar quebrar depois esconderia isso atrás de uma frase inteira.
      if (!useSettingsStore.getState().settings.elevenLabsApiKey.trim()) {
        set({
          dictationError:
            'para conversar por voz, configure a chave da ElevenLabs e escolha uma voz em Diagnóstico › Voz',
        })
        return
      }

      set({ dictationError: null })
      await get().startDictation()
      if (!get().isDictating) return

      set({ isConversing: true })
      runConversationLoop()
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
