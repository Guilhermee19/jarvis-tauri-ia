'use client'

import { useChatStore, useSensorStore, useSettingsStore } from '@/stores'

/** De quem é a vez no modo conversa. */
export type ConversationStatus = 'ouvindo' | 'pensando' | 'falando'

/**
 * Ditado por ALTERNÂNCIA: clica, fala, clica de novo e o texto aparece no campo.
 *
 * Não é "segurar para falar", apesar do que dizia este comentário: um botão que só
 * vale enquanto o ponteiro está pressionado não dá para operar pelo teclado. O
 * raciocínio inteiro está no `toggleMic` do `ChatInput`.
 *
 * A assinatura mudou em relação ao stub que existia aqui. Ele previa transcrições
 * PARCIAIS chegando por evento (`jarvis://transcript`), o que faz sentido para escuta
 * contínua com wake word — não é o caso. Aqui a gravação tem começo e fim marcados
 * pelo dedo do usuário, e a transcrição é uma pergunta com uma resposta: `stop()`
 * devolve o texto. Um evento teria um produtor e um consumidor já em chamada direta.
 *
 * O estado todo mora no `sensorStore` porque o dono do gravador precisa ser um só —
 * o microfone da bancada de diagnóstico e este botão disputariam o dispositivo.
 */
export function useVoiceInput() {
  const isRecording = useSensorStore((state) => state.isDictating)
  const isTranscribing = useSensorStore((state) => state.isTranscribing)
  const start = useSensorStore((state) => state.startDictation)
  const stop = useSensorStore((state) => state.stopDictation)
  // O pico do microfone, para o botão poder PROVAR que está ouvindo. Sem isso, mic
  // mudo no painel do Windows e mic funcionando são a mesma tela — e a diferença só
  // aparecia segundos depois, como "não ouvi nada".
  const level = useSensorStore((state) => state.micLevel)
  const ttsLevel = useSensorStore((state) => state.ttsLevel)
  const error = useSensorStore((state) => state.dictationError)
  const clearError = useSensorStore((state) => state.clearDictationError)

  // O modo conversa é o mesmo microfone com o laço ligado, então vem pelo mesmo
  // hook: quem desenha o botão de falar é quem desenha o de conversar.
  const isConversing = useSensorStore((state) => state.isConversing)
  const toggleConversation = useSensorStore((state) => state.toggleConversation)

  // De quem é a vez. Derivado do chat, e não guardado no `sensorStore`, porque
  // pensar e falar são estados da RESPOSTA — copiá-los para cá criaria duas
  // versões da mesma verdade, e uma delas ficaria para trás.
  const isThinking = useChatStore((state) => state.isTyping)
  const isSpeaking = useChatStore((state) => state.isSpeaking)
  // O motor decide a calibração do medidor logo abaixo — os dois normalizam diferente.
  const motorDeVoz = useSettingsStore((state) => state.settings.ttsEngine)
  const conversationStatus: ConversationStatus = isSpeaking
    ? 'falando'
    : isThinking
      ? 'pensando'
      : 'ouvindo'

  /**
   * O nível do áudio que importa AGORA, seja ele de entrada ou de saída.
   *
   * Uma grandeza só, porque quem desenha quer uma: o núcleo do HUD pulsa com o Jarvis
   * ouvindo e com ele falando, e as duas coisas nunca acontecem ao mesmo tempo — o
   * microfone fecha antes da fala começar, justamente para ele não ouvir a si mesmo.
   *
   * Continua na escala linear: a CURVA é de quem desenha, e os medidores deste projeto
   * aplicam `Math.sqrt` para tirar a fala do fundo da escala. Aplicá-la aqui esconderia
   * essa decisão de quem lê o componente.
   *
   * O que é feito aqui é outra coisa — **igualar as duas fontes**. Uma voz perto do
   * microfone encosta em 1; a fala sintetizada não chega lá. Sem compensar, o núcleo pulsa
   * visivelmente menos quando o Jarvis fala do que quando ele ouve, e essa diferença não
   * diz nada sobre o áudio.
   *
   * **Os dois números saíram de medir**, e são bem diferentes:
   *
   * - **Piper: 1,0.** Ele normaliza a saída, e a frase de teste bateu pico exatamente 1,000.
   * - **Chatterbox: 0,28.** Uma frase de 4,4 s gerada em WAV bateu 0,264 — e ainda depende
   *   do clipe, porque o modelo clona o VOLUME junto com a voz.
   *
   * Por isso a constante segue o motor, e não é um número só. Usar o 0,28 com o Piper faria
   * o núcleo saturar em toda sílaba; usar 1,0 com o Chatterbox o deixaria quase parado. É a
   * mesma armadilha que o 0,6 da ElevenLabs já causou uma vez, e o que a evita é ela ser
   * derivada de `ttsEngine` em vez de escrita à mão.
   *
   * Se trocar o clipe do Chatterbox e o núcleo pulsar de menos, o número novo sai de
   * `cargo test --lib -- --ignored --nocapture fala_de_verdade`, que imprime o pico.
   */
  const PICO_TIPICO_DA_FALA = motorDeVoz === 'piper' ? 1 : 0.28
  const nivelDeAudio = isSpeaking ? Math.min(1, ttsLevel / PICO_TIPICO_DA_FALA) : level

  return {
    isRecording,
    nivelDeAudio,
    isTranscribing,
    start,
    stop,
    level,
    error,
    clearError,
    isConversing,
    conversationStatus,
    toggleConversation,
  }
}
