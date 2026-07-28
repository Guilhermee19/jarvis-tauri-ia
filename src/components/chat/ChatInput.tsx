'use client'

import { useRef, useState, type KeyboardEvent } from 'react'
import { Button } from '@/components/ui/Button'

interface ChatInputProps {
  onSend: (content: string) => void
  disabled: boolean
}

export function ChatInput({ onSend, disabled }: ChatInputProps) {
  const [value, setValue] = useState('')
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  function submit() {
    const trimmed = value.trim()
    if (!trimmed || disabled) return
    onSend(trimmed)
    setValue('')
    textareaRef.current?.focus()
  }

  function onKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    // Enter envia; Shift+Enter quebra linha.
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault()
      submit()
    }
  }

  return (
    <div className="border-border-soft bg-surface/70 border-t px-3 py-3 backdrop-blur-sm">
      <div className="mx-auto flex w-full max-w-[560px] items-end gap-2">
        <textarea
          ref={textareaRef}
          rows={1}
          value={value}
          onChange={(event) => setValue(event.target.value)}
          onKeyDown={onKeyDown}
          placeholder="Fale com o Jarvis…"
          className="border-border-soft bg-base text-content placeholder:text-muted/60 focus:border-accent scroll-thin max-h-28 min-h-[38px] flex-1 resize-none rounded-lg border px-3 py-2 text-sm focus:outline-none"
        />
        {/* O botão de microfone entra aqui na v0.3, ligado ao hook `useVoiceInput`. */}
        <Button
          onClick={submit}
          disabled={disabled || value.trim().length === 0}
          className="h-[38px]"
        >
          Enviar
        </Button>
      </div>
    </div>
  )
}
