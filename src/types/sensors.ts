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

/**
 * Um clipe de voz cadastrado no servidor local — o que o Chatterbox clona.
 *
 * O `id` é o nome do arquivo no servidor, que é também o que vai para
 * `ttsVoiceJarvis`/`ttsVoiceUltron`. O `name` é o mesmo sem a extensão, e `description`
 * hoje é sempre `null`: o servidor não guarda metadado nenhum sobre os clipes.
 */
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

/**
 * Uma resolução que a câmera declara suportar.
 *
 * A lista vem do dispositivo, não de constantes: oferecer "1080p" numa webcam que só
 * chega a 720p faria a escolha ser trocada em silêncio pelo formato mais próximo.
 */
export interface WebcamResolution {
  width: number
  height: number
  /** Maior taxa oferecida NESTA resolução — costuma cair conforme ela sobe. */
  maxFps: number
}

export interface MonitorInfo {
  id: number
  name: string
  width: number
  height: number
  isPrimary: boolean
}
