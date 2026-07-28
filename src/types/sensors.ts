/**
 * Espelha os tipos de `src-tauri/src/core/voice/` e `src-tauri/src/core/automation/`
 * (serde em camelCase).
 *
 * São os contratos que a v0.2+ vai reusar: o agente chama os mesmos comandos que a
 * tela de diagnóstico chama, e recebe exatamente estas formas.
 */

/** O que sobra de uma gravação. `path` é o WAV que a transcrição vai ler. */
export interface Recording {
  path: string
  durationSeconds: number
  sampleRate: number
  sampleCount: number
}

/** Voz do catálogo da ElevenLabs. */
export interface Voice {
  id: string
  name: string
  description: string | null
}

/** Imagem já como `data:` URL — vai direto no `src` de uma `<img>`. */
export interface CapturedImage {
  dataUrl: string
  width: number
  height: number
}

export interface MonitorInfo {
  id: number
  name: string
  width: number
  height: number
  isPrimary: boolean
}
