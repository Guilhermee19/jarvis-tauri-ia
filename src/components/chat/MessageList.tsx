'use client'

import { useEffect, useRef } from 'react'
import { MessageBubble } from './MessageBubble'
import { TypingIndicator } from './TypingIndicator'
import type { ChatMessage } from '@/types'

interface MessageListProps {
  messages: ChatMessage[]
  isTyping: boolean
  assistantName: string
}

export function MessageList({ messages, isTyping, assistantName }: MessageListProps) {
  const bottomRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages, isTyping])

  return (
    <div className="scroll-thin flex flex-1 flex-col overflow-y-auto">
      {/* Coluna centralizada: a janela é larga, mas linha de conversa comprida
          cansa de ler. `flex-1` no lugar de `h-full` evita rolagem fantasma. */}
      <div className="mx-auto flex w-full max-w-[560px] flex-1 flex-col gap-3 px-3 py-4">
        {messages.length === 0 && !isTyping ? (
          <div className="flex flex-1 flex-col items-center justify-center gap-2 px-6 text-center">
            <p className="text-content text-sm font-medium">Olá, eu sou o {assistantName}.</p>
            <p className="text-muted text-xs">
              Ainda estou no modo esqueleto — as respostas são simuladas.
            </p>
          </div>
        ) : null}

        {messages.map((message) => (
          <MessageBubble key={message.id} message={message} />
        ))}

        {isTyping ? <TypingIndicator assistantName={assistantName} /> : null}

        <div ref={bottomRef} />
      </div>
    </div>
  )
}
