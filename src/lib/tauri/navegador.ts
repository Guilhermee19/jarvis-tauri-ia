import type { AreaDoNavegador, EstadoDoNavegador } from '@/types'
import { call } from './client'

/** Wrappers de `src-tauri/src/commands/navegador.rs`. */

/**
 * Abre um endereço numa aba nova.
 *
 * Aceita o que a pessoa disser — "youtube", "youtube.com", a URL inteira: o Rust usa a
 * MESMA normalização de quando manda para o navegador do sistema.
 */
export function browserOpen(url: string): Promise<EstadoDoNavegador> {
  return call<EstadoDoNavegador>('browser_open', { url })
}

/** Abre uma busca numa aba nova. */
export function browserSearch(query: string): Promise<EstadoDoNavegador> {
  return call<EstadoDoNavegador>('browser_search', { query })
}

export function browserState(): Promise<EstadoDoNavegador> {
  return call<EstadoDoNavegador>('browser_state')
}

export function browserSelect(id: string): Promise<EstadoDoNavegador> {
  return call<EstadoDoNavegador>('browser_select', { id })
}

export function browserClose(id: string): Promise<EstadoDoNavegador> {
  return call<EstadoDoNavegador>('browser_close', { id })
}

/** Manda uma aba existente para outro endereço — o que a barra de endereço faz. */
export function browserNavigate(id: string, url: string): Promise<EstadoDoNavegador> {
  return call<EstadoDoNavegador>('browser_navigate', { id, url })
}

/** Volta (`-1`) ou avança (`1`) no histórico da aba. */
export function browserHistory(id: string, passo: number): Promise<void> {
  return call<void>('browser_history', { id, passo })
}

/**
 * Diz ao Rust onde desenhar as abas. `null` esconde todas.
 *
 * **É obrigatório chamar**, e chamar de novo a cada movimento: o webview é uma camada
 * nativa acima do HTML, então ele não segue o painel sozinho. Sem o `null` ao fechar, ele
 * continuaria desenhado sobre um painel que já não existe.
 */
export function browserBounds(area: AreaDoNavegador | null): Promise<void> {
  return call<void>('browser_bounds', { area })
}

/**
 * Manda a página para o navegador de verdade.
 *
 * A saída de emergência: login com senha salva, extensão, imprimir. O navegador embutido é
 * simples de propósito, e o que ele não faz precisa ter para onde ir.
 */
export function browserExternal(id: string): Promise<void> {
  return call<void>('browser_external', { id })
}
