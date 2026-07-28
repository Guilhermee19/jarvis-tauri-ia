'use client'

/**
 * STUB — entra na v0.3 (voz).
 *
 * A detecção da palavra-gatilho roda em background no Rust (`core/voice`), não aqui:
 * este hook só vai refletir o estado ("ouvindo", última detecção) e reagir ao evento
 * `jarvis://wake-word` para trazer a janela para frente.
 */
export interface WakeWordState {
  isListening: boolean
  lastDetectedAt: number | null
}

export function useWakeWord(): WakeWordState {
  return { isListening: false, lastDetectedAt: null }
}
