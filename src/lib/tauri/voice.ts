import type { Recording, Voice } from '@/types'
import { call } from './client'

/** Wrappers de `src-tauri/src/commands/voice.rs`. */

/**
 * Enquanto grava, o backend emite `JarvisEvent.MicLevel` ~20×/s com o pico do
 * intervalo — é o que alimenta o medidor de volume.
 */
export function startRecording(): Promise<void> {
  return call<void>('start_recording')
}

/** Fecha o microfone e devolve o WAV gravado em disco. */
export function stopRecording(): Promise<Recording> {
  return call<Recording>('stop_recording')
}

export function isRecording(): Promise<boolean> {
  return call<boolean>('is_recording')
}

export function listVoices(): Promise<Voice[]> {
  return call<Voice[]>('list_voices')
}

/**
 * `voiceId` é opcional de propósito: sem ele o backend usa a voz das configurações.
 * É essa assinatura que o agente da v0.2 vai chamar — `speakText(resposta)` e pronto,
 * sem precisar saber que existe catálogo de vozes.
 */
export function speakText(text: string, voiceId?: string): Promise<void> {
  return call<void>('speak_text', { text, voiceId: voiceId ?? null })
}
