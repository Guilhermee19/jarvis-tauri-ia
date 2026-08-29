'use client'

import { useChatStore, useSensorStore } from '@/stores'

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
   * O que é feito aqui é outra coisa — **igualar as duas fontes**. Medido pelo
   * `fala_de_verdade` em quatro frases (de um sussurro a um grito), o pico do MP3 da
   * ElevenLabs ficou entre **0,52 e 0,71**, enquanto uma voz perto do microfone encosta em
   * 1. Sem compensar, o núcleo pulsa visivelmente menos quando o Jarvis fala do que
   * quando ele ouve — e essa diferença não diz nada sobre o áudio, é só o encoder.
   *
   * O `min` não é enfeite: a frase mais alta bateu 1,18 depois da divisão.
   */
  const PICO_TIPICO_DA_FALA = 0.6
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
