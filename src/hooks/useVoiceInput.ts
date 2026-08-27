'use client'

import { useSensorStore } from '@/stores'

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
  const error = useSensorStore((state) => state.dictationError)
  const clearError = useSensorStore((state) => state.clearDictationError)

  return { isRecording, isTranscribing, start, stop, level, error, clearError }
}
