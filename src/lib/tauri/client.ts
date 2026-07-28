import { invoke } from '@tauri-apps/api/core'

/**
 * Camada única de acesso ao backend. Todo `invoke()` do app passa por aqui, então
 * logging, retry ou telemetria entram em um lugar só.
 */

export class TauriCommandError extends Error {
  constructor(
    readonly command: string,
    readonly cause: unknown,
  ) {
    super(`Comando "${command}" falhou: ${normalizeCause(cause)}`)
    this.name = 'TauriCommandError'
  }
}

function normalizeCause(cause: unknown): string {
  if (typeof cause === 'string') return cause
  if (cause instanceof Error) return cause.message
  return JSON.stringify(cause)
}

/**
 * `true` só quando a UI está rodando dentro da janela do Tauri.
 * Abrir `localhost:3000` direto no navegador (útil para mexer em CSS) cai no `false`.
 */
export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) {
    throw new TauriCommandError(command, 'app fora do runtime do Tauri (rode `npm run tauri dev`)')
  }

  try {
    return await invoke<T>(command, args)
  } catch (cause) {
    throw new TauriCommandError(command, cause)
  }
}
