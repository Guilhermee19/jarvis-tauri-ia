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
  /** Modelo que interpreta, conversa E enxerga. Vazio DESLIGA o intérprete. */
  ollamaModel: string
  /** Pasta da memória em markdown. Vazio = `memoria/` no projeto. */
  memoriaPath: string
  /** Chave do Brave Search. Vazio = Wikipedia (sem chave, mas só enciclopédia). */
  braveApiKey: string
  /** Credenciais do Spotify. Vazias = "toque X" abre a busca em vez de tocar. */
  spotifyClientId: string
  spotifyClientSecret: string
  /**
   * Resolução pedida à webcam. `0` em qualquer um dos dois = automático (o formato
   * mais perto de 640×480). É um pedido: a câmera decide o que consegue entregar.
   */
  webcamWidth: number
  webcamHeight: number
  /** Espelhar a imagem na tela (visão de selfie). Só exibição — não muda os bytes. */
  webcamMirror: boolean
}

export const DEFAULT_SETTINGS: AppSettings = {
  anthropicApiKey: '',
  assistantName: 'Jarvis',
  elevenLabsApiKey: '',
  ttsVoiceId: '',
  ollamaUrl: 'http://localhost:11434',
  ollamaModel: 'qwen2.5vl:3b',
  memoriaPath: '',
  braveApiKey: '',
  spotifyClientId: '',
  spotifyClientSecret: '',
  webcamWidth: 0,
  webcamHeight: 0,
  webcamMirror: false,
}
