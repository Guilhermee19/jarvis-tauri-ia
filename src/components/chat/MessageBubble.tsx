'use client'

import { cn, formatTime } from '@/lib/utils'
import type { ChatMessage } from '@/types'

/** Bolhas por papel. `system` é o log de gatilho e ação que o agente empurra. */
export function MessageBubble({ message }: { message: ChatMessage }) {
  const isUser = message.role === 'user'

  // Sem bolha e sem lado: isto é registro da máquina, não fala. A borda tracejada é
  // o que separa as duas coisas sem precisar inventar uma cor nova.
  if (message.role === 'system') {
    return (
      <div className="border-border-soft bg-surface/40 rounded-md border border-dashed px-3 py-2">
        <div className="text-accent pb-1 text-[9px] tracking-[0.22em] uppercase">
          Log · {formatTime(message.timestamp)}
        </div>
        {/* Minúsculo e monoespaçado de propósito: alinha as colunas do trace e deixa
            o olho pular por cima quando não interessa. */}
        <pre className="text-muted font-mono text-[10px] leading-relaxed whitespace-pre-wrap">
          {message.content}
        </pre>
      </div>
    )
  }

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
