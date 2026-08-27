'use client'

import { ChatInput } from './ChatInput'
import { MessageList } from './MessageList'
import { useChat } from '@/hooks/useChat'
import { useSettingsStore } from '@/stores'

/** Corpo do painel de conversa — o chrome da janelinha é do `FloatingPanel`. */
export function ChatPanel() {
  const { messages, isTyping, error, send } = useChat()
  const assistantName = useSettingsStore((state) => state.settings.assistantName)

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <MessageList messages={messages} isTyping={isTyping} assistantName={assistantName} />

      {error ? (
        <p className="border-danger/30 bg-danger/10 text-danger border-t px-3 py-2 text-[11px]">
          {error}
        </p>
      ) : null}

      <ChatInput
        onSend={(content) => void send(content)}
        disabled={isTyping}
        assistantName={assistantName}
      />
    </div>
  )
}
