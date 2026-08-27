import { create } from 'zustand'
import { clearHistory, getHistory, sendMessage, speakText, stopSpeaking } from '@/lib/tauri'
import { useSettingsStore } from './settingsStore'
import type { ChatMessage } from '@/types'

/**
 * O histórico canônico é do backend (`AppState` no Rust). Esta store é um espelho
 * para a UI — por isso `loadHistory` sobrescreve tudo em vez de fazer merge.
 */
interface ChatState {
  messages: ChatMessage[]
  isTyping: boolean
  /**
   * Ele está falando a resposta agora.
   *
   * Mora aqui, e não no `sensorStore` junto do microfone, porque a fala acompanha a
   * RESPOSTA — vale para o que foi digitado igual ao que foi falado. O modo conversa
   * lê esta flag em vez de manter uma cópia própria.
   */
  isSpeaking: boolean
  error: string | null
  loadHistory: () => Promise<void>
  /**
   * Envia, espera a resposta e a FALA — só volta quando ele calou.
   *
   * Falar aqui, e não em quem chama, é o que faz a voz valer nos dois caminhos: por
   * texto e no modo conversa. E o `await` até o fim da fala é o que deixa o laço da
   * conversa saber quando pode reabrir o microfone.
   *
   * Devolve o texto da resposta, ou string vazia se nada foi enviado ou algo falhou.
   */
  send: (content: string) => Promise<string>
  clear: () => Promise<void>
}

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

/**
 * Fala a resposta, e só volta quando ela termina.
 *
 * Sem key configurada ele fica calado e SEM ERRO: voz é opcional, e um aviso vermelho
 * a cada mensagem digitada seria ruído por uma coisa que ninguém pediu. Quem liga o
 * modo conversa aí sim recebe a recusa na hora do clique, porque ali a voz é o ponto.
 *
 * Só a resposta é falada. O log de ação (papel `system`) chega junto no
 * `loadHistory` e fica só escrito — ninguém quer ouvir `open_site url=https://…`.
 */
async function falar(texto: string, set: (state: Partial<ChatState>) => void, falando: boolean) {
  if (!texto.trim()) return
  if (!useSettingsStore.getState().settings.elevenLabsApiKey.trim()) return

  // Mandar uma mensagem nova enquanto ele fala a anterior CORTA a anterior. Duas
  // falas sobrepostas seriam ininteligíveis, e a resposta que interessa é a última.
  // ponytail: corta pela flag, então a anterior morre em até 100 ms — sobreposição
  // só apareceria se a síntese da nova voltasse mais rápido que isso, o que a rede
  // não permite.
  if (falando) await stopSpeaking().catch(() => undefined)

  set({ isSpeaking: true })
  try {
    await speakText(texto)
  } catch (error) {
    set({ error: describeError(error) })
  } finally {
    set({ isSpeaking: false })
  }
}

export const useChatStore = create<ChatState>((set, get) => ({
  messages: [],
  isTyping: false,
  isSpeaking: false,
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

      // Depois do `loadHistory`: a resposta aparece escrita e ENTÃO ele começa a
      // falar. Falar primeiro deixaria a tela um passo atrás da voz.
      await falar(message.content, set, get().isSpeaking)
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
