import { create } from 'zustand'
import { clearHistory, getHistory, sendMessage } from '@/lib/tauri'
import type { ChatMessage } from '@/types'

/**
 * O histórico canônico é do backend (`AppState` no Rust). Esta store é um espelho
 * para a UI — por isso `loadHistory` sobrescreve tudo em vez de fazer merge.
 */
interface ChatState {
  messages: ChatMessage[]
  isTyping: boolean
  error: string | null
  loadHistory: () => Promise<void>
  /**
   * Devolve o texto da resposta, ou string vazia se nada foi enviado ou algo falhou.
   *
   * Quem fala em voz alta precisa da resposta E SÓ DELA: o `loadHistory` logo abaixo
   * traz também o log de ação (papel `system`), que é registro para ler, não fala.
   */
  send: (content: string) => Promise<string>
  clear: () => Promise<void>
}

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export const useChatStore = create<ChatState>((set, get) => ({
  messages: [],
  isTyping: false,
  error: null,

  loadHistory: async () => {
    try {
      set({ messages: await getHistory(), error: null })
    } catch (error) {
      set({ error: describeError(error) })
    }
  },

  send: async (content: string) => {
    const trimmed = content.trim()
    if (!trimmed || get().isTyping) return ''

    // Bolha otimista: o backend gera o id definitivo, que chega no próximo `loadHistory`.
    const optimistic: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content: trimmed,
      timestamp: Date.now(),
    }
    set((state) => ({ messages: [...state.messages, optimistic], isTyping: true, error: null }))

    try {
      // Uma jogada do agente pode empurrar DUAS mensagens no histórico: o log do
      // gatilho e a resposta. Recarregar em vez de dar append mantém o espelho fiel
      // — e de quebra troca a bolha otimista pela versão com o id do backend.
      const { message } = await sendMessage(trimmed)
      await get().loadHistory()
      set({ isTyping: false })
      return message.content
    } catch (error) {
      set({ isTyping: false, error: describeError(error) })
      return ''
    }
  },

  clear: async () => {
    try {
      await clearHistory()
      set({ messages: [], error: null })
    } catch (error) {
      set({ error: describeError(error) })
    }
  },
}))
