'use client'

import { useChatStore } from '@/stores'

/** Recorte da store de chat que a UI de conversa consome. */
export function useChat() {
  const messages = useChatStore((state) => state.messages)
  const isTyping = useChatStore((state) => state.isTyping)
  const error = useChatStore((state) => state.error)
  const send = useChatStore((state) => state.send)
  const clear = useChatStore((state) => state.clear)

  return { messages, isTyping, error, send, clear }
}
