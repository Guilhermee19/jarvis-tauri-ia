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
   * O que é feito aqui é outra coisa — **igualar as duas fontes**. Uma voz perto do
   * microfone encosta em 1; a fala sintetizada não chega lá. Sem compensar, o núcleo pulsa
   * visivelmente menos quando o Jarvis fala do que quando ele ouve, e essa diferença não
   * diz nada sobre o áudio.
   *
   * **0,28 saiu de medir**, no Chatterbox com um clipe de referência real: uma frase de 4,4 s
   * gerada em WAV bateu pico 0,264. O valor anterior era 0,6, medido no encoder MP3 da
   * ElevenLabs — mantido por engano, o núcleo do HUD pulsaria com menos da METADE da
   * amplitude, e ninguém ligaria isso a uma constante de outro motor.
   *
   * **Depende do seu clipe.** O modelo clona o volume junto com a voz: uma referência
   * gravada baixa gera fala baixa. Se o núcleo pulsar de menos, o número certo sai de
   * `cargo test --lib -- --ignored --nocapture fala_de_verdade`, que imprime o pico da sua.
   */
  const PICO_TIPICO_DA_FALA = 0.28
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
