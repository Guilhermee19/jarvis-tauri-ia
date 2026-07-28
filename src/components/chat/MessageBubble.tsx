'use client'

import { cn, formatTime } from '@/lib/utils'
import type { ChatMessage } from '@/types'

/** Bolhas por papel. Quando o agente real chegar, `system`/tool-use entram aqui. */
export function MessageBubble({ message }: { message: ChatMessage }) {
  const isUser = message.role === 'user'

  return (
    <div className={cn('flex w-full', isUser ? 'justify-end' : 'justify-start')}>
      <div className={cn('flex max-w-[85%] flex-col gap-1', isUser ? 'items-end' : 'items-start')}>
        <div
          className={cn(
            'rounded-2xl px-3.5 py-2 text-sm leading-relaxed whitespace-pre-wrap',
            isUser
              ? 'bg-accent-strong rounded-br-md text-white'
              : 'border-border-soft bg-surface text-content rounded-bl-md border',
          )}
        >
          {message.content}
        </div>
        <span className="text-muted px-1 text-[10px]">{formatTime(message.timestamp)}</span>
      </div>
    </div>
  )
}
