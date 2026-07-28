'use client'

/**
 * STUB — entra na v0.3 (voz).
 *
 * Vai controlar a captura de áudio e o STT: `start()` chama um comando que liga o
 * pipeline em `src-tauri/src/core/voice`, e as transcrições parciais chegam por
 * evento (`jarvis://transcript`), do mesmo jeito que `useTrayEvents` escuta a bandeja.
 * A assinatura já está aqui para o `ChatInput` poder plugar sem refatorar.
 */
export interface VoiceInputState {
  isSupported: boolean
  isRecording: boolean
  transcript: string
}

export function useVoiceInput(): VoiceInputState {
  return { isSupported: false, isRecording: false, transcript: '' }
}
