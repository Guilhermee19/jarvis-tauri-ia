/** Espelha `AppSettings` de `src-tauri/src/config/mod.rs` (serde em camelCase). */
export interface AppSettings {
  /** Guardada em disco sem validação nesta versão. */
  anthropicApiKey: string
  /** Vira parte do system prompt quando o agente real entrar. */
  assistantName: string
  /** Key da ElevenLabs, usada pelo TTS. */
  elevenLabsApiKey: string
  /** Vazio = o backend usa a primeira voz da conta. */
  ttsVoiceId: string
  /** Onde o Ollama escuta. Aponta para outra máquina se o Ollama não roda aqui. */
  ollamaUrl: string
  /** Modelo que interpreta os comandos. Vazio DESLIGA o intérprete. */
  ollamaModel: string
}

export const DEFAULT_SETTINGS: AppSettings = {
  anthropicApiKey: '',
  assistantName: 'Jarvis',
  elevenLabsApiKey: '',
  ttsVoiceId: '',
  ollamaUrl: 'http://localhost:11434',
  ollamaModel: 'qwen2.5:3b',
}
