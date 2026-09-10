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
import { comandoEnderecado } from '@/lib/dictation'
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
 * De quanto em quanto tempo a escuta olha o medidor do microfone.
 *
 * É um `setInterval` lendo `micLevel`, e não uma reação à mudança do valor, porque
 * em silêncio o pico chega 0 repetido — e o zustand não notifica quem seleciona um
 * valor igual ao anterior. Reagir à mudança perderia exatamente o caso que importa,
 * que é justamente o silêncio parado.
 *
 * 200 ms para uma fonte que chega a 20 Hz — e é por isso que o que se lê aqui é o
 * PICO acumulado desde a última olhada, e não o `micLevel` daquele instante. Amostrar
 * o valor pontual joga fora três de cada quatro medidas, e as jogadas fora são
 * justamente as sílabas fortes: a fala vira uma sequência de picos e vales de ~50 ms,
 * e o sorteio de qual deles o timer pega decidia se a frase existia ou não. Era isso
 * que obrigava a falar alto E devagar — arrastar a sílaba é o único jeito de garantir
 * que ela sobreviva à amostragem.
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
   * uma FASE da resposta — ele começa antes do som existir, enquanto o modelo ainda
   * está sintetizando.
   */
  ttsLevel: number
  micError: string | null
  lastRecording: Recording | null
  toggleMic: () => Promise<void>
  setMicLevel: (level: number) => void
  setTtsLevel: (level: number) => void

  /**
   * O gravador do turno de voz — um trecho de fala entre dois silêncios. Mora AQUI,
   * junto do `toggleMic`, porque o dono do gravador tem que ser um só: se a escuta
   * chamasse `startRecording` por conta própria com o microfone da bancada ligado, o
   * backend responderia "já existe uma gravação em andamento".
   */
  isDictating: boolean
  isTranscribing: boolean
  /**
   * Erro da escuta, SEPARADO do `micError` da bancada.
   *
   * Separado porque os dois têm donos diferentes: o `micError` é da gravação de teste
   * do diagnóstico, e este é do microfone que fica ouvindo o tempo todo. Juntá-los
   * faria um erro de lá acusar o botão de cá, e vice-versa.
   */
  dictationError: string | null
  startDictation: () => Promise<void>
  /** Devolve o transcrito, ou string vazia se nada foi ouvido ou algo falhou. */
  stopDictation: () => Promise<string>
  clearDictationError: () => void

  /**
   * Escuta contínua: o microfone fica aberto ouvindo TUDO, o silêncio marca o fim de
   * cada frase, e só a que começa pelo nome do assistente vira comando.
   *
   * É o único jeito de falar com ele por voz — não há mais botão de "falar" nem de
   * "conversar" no painel de chat. O gatilho é o nome, como se chama qualquer pessoa
   * numa sala: sem ele a frase é conversa alheia e é descartada sem resposta.
   *
   * Mora aqui porque o gravador tem um dono só. E o laço fica na store, e não num
   * hook, para sobreviver a fechar o painel de chat — desligar a janelinha não é
   * dizer "pare de me ouvir".
   */
  isListening: boolean
  toggleListening: () => Promise<void>
  /**
   * Liga ou desliga a escuta sem depender de qual é o estado agora.
   *
   * Existe pelo mesmo motivo do `setWebcam` ao lado do `toggleWebcam`: quem chama na
   * abertura do app quer ESCUTA LIGADA, não "o contrário do que estiver valendo" — e
   * um `toggle` chamado duas vezes (o efeito que o React roda em dobro no modo estrito
   * do `dev`) desligaria justamente o que a primeira chamada ligou.
   */
  setListening: (on: boolean) => Promise<void>
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
let listeningTimer: ReturnType<typeof setInterval> | null = null

function stopListeningLoop() {
  if (listeningTimer !== null) clearInterval(listeningTimer)
  listeningTimer = null
}

/**
 * O maior pico que o microfone mandou desde a última olhada do VAD.
 *
 * Fora do estado pela mesma razão do `listeningTimer`: ninguém desenha isto. Quem
 * desenha lê o `micLevel`, que continua sendo o valor do instante.
 */
let picoAcumulado = 0

/** Devolve o pico e zera a conta — a próxima janela começa limpa. */
function consumirPico(): number {
  const pico = picoAcumulado
  picoAcumulado = 0
  return pico
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
   * O laço da escuta: amostra o medidor, e quando o VAD diz que a frase acabou,
   * encadeia transcrever → decidir se era para ele → responder → voltar a ouvir.
   *
   * `ocupado` existe porque o turno leva SEGUNDOS (Whisper + Ollama + Chatterbox) e
   * o timer continua disparando durante todos eles. Sem a trava, a amostra seguinte
   * tentaria fechar um turno que já está sendo fechado.
   */
  function runListeningLoop() {
    let turno: TurnoVad = iniciarTurno(Date.now())
    let ocupado = false
    consumirPico()

    listeningTimer = setInterval(() => {
      if (ocupado || !get().isListening) return

      const passo = avaliarTurno(turno, consumirPico(), Date.now())
      turno = passo.turno
      if (passo.decisao === 'ouvindo') return

      ocupado = true
      void fecharTurno(passo.decisao).finally(() => {
        // O fundo da sala sobrevive ao turno: ele é do ambiente, não da gravação, e
        // reaprendê-lo do zero a cada frase custaria as primeiras amostras da frase
        // seguinte. O pico, ao contrário, é zerado — o que entrou enquanto ele
        // pensava e falava não é fala de ninguém para este turno.
        turno = iniciarTurno(Date.now(), turno.ruido)
        consumirPico()
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
        // Desligar a escuta no meio de um turno descarta a frase de propósito: o
        // clique foi "pare", e mandar a última coisa ouvida seria o contrário.
        if (!get().isListening) return

        // O NOME é o gatilho, e é ele que separa "falou com o Jarvis" de "falou
        // perto do Jarvis". Com o microfone aberto o tempo todo, tudo que é dito na
        // sala chega até aqui — sem esse filtro, uma conversa entre duas pessoas
        // viraria uma fila de comandos. O que não é endereçado morre aqui, calado.
        //
        // ponytail: todo trecho de fala paga o Whisper antes de ser descartado — a
        // única forma de saber se o nome foi dito é transcrevendo. Uma palavra-gatilho
        // de verdade (Porcupine, openWakeWord) resolveria isso dentro do `mic.rs`, sem
        // mudar nada daqui.
        const comando = comandoEnderecado(
          ouvido,
          useSettingsStore.getState().settings.assistantName,
        )

        // O `send` responde E fala — só volta quando ele calou, que é exatamente
        // quando o microfone pode reabrir sem ouvir a si mesmo.
        if (comando) await useChatStore.getState().send(comando)
      }

      if (!get().isListening) return
      await get().startDictation()

      // Não conseguiu reabrir o microfone (dispositivo arrancado, permissão
      // revogada): desliga a escuta em vez de girar para sempre sem gravar nada. O
      // `startDictation` já deixou o motivo em `dictationError`.
      if (!get().isDictating) {
        stopListeningLoop()
        set({ isListening: false })
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
      // O outro lado da recusa que o `startDictation` já fazia: com a escuta ligada o
      // gravador é dela, e pedir de novo esbarraria no "já existe uma gravação em
      // andamento" do backend — um erro que não diz onde desligar.
      if (!get().isMicOn && get().isListening) {
        set({
          micError: 'o microfone está ocupado pela escuta — desligue o ícone do microfone na barra',
        })
        return
      }
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

    setMicLevel: (level) => {
      picoAcumulado = Math.max(picoAcumulado, level)
      set({ micLevel: level })
    },
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
        // Erro vira aviso e string vazia: um turno perdido não pode derrubar a
        // escuta só porque o Whisper não estava lá.
        set({ dictationError: describe(cause) })
        return ''
      } finally {
        set({ isTranscribing: false })
      }
    },

    isListening: false,

    toggleListening: async () => get().setListening(!get().isListening),

    setListening: async (on: boolean) => {
      if (get().isListening === on) return

      if (!on) {
        stopListeningLoop()
        set({ isListening: false })
        // Calar vem ANTES de soltar o microfone: quem clicou em desligar quer
        // silêncio agora, não quando a frase em curso terminar.
        await stopSpeaking().catch(() => undefined)
        await get().stopDictation()
        return
      }

      // Sem clipe de voz ele ouve e executa do mesmo jeito, só responde por escrito
      // na janelinha de conversa. Recusar aqui trocaria uma escuta útil por nenhuma
      // — a voz é o acabamento da resposta, não a condição para ouvir.
      set({ dictationError: null })
      await get().startDictation()
      if (!get().isDictating) return

      set({ isListening: true })
      runListeningLoop()
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
