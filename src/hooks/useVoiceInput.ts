'use client'

import { useSensorStore } from '@/stores'

/**
 * Ditado por "segurar para falar": aperta, fala, solta e o texto aparece no campo.
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

  return { isRecording, isTranscribing, start, stop }
}
