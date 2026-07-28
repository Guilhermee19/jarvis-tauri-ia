/** Espelha `AppSettings` de `src-tauri/src/config/mod.rs` (serde em camelCase). */
export interface AppSettings {
  /** Guardada em disco sem validação nesta versão. */
  anthropicApiKey: string
  /** Vira parte do system prompt quando o agente real entrar. */
  assistantName: string
}

export const DEFAULT_SETTINGS: AppSettings = {
  anthropicApiKey: '',
  assistantName: 'Jarvis',
}
