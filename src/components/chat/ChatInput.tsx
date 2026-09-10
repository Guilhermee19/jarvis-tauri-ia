'use client'

import { useRef, useState, type KeyboardEvent } from 'react'
import { Button } from '@/components/ui/Button'

interface ChatInputProps {
  onSend: (content: string) => void
  disabled: boolean
  /** Só para o texto do campo — falar com ele é pelo microfone da barra. */
  assistantName: string
}

/**
 * O campo de digitar. **Sem botão de microfone**, de propósito.
 *
 * Ele tinha dois — "falar" e "conversar por voz" —, e os dois eram a mesma pergunta
 * feita duas vezes: quando é que ele está me ouvindo. Agora existe um interruptor só,
 * o microfone da barra de ícones: ligado, ele ouve tudo e obedece ao que vem depois do
 * nome dele. Um botão de falar aqui dentro seria um terceiro jeito de fazer a mesma
 * coisa — e um que só funciona com a janelinha de conversa aberta.
 */
export function ChatInput({ onSend, disabled, assistantName }: ChatInputProps) {
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
          placeholder={`Fale com o ${assistantName}…`}
          className="border-border-soft bg-base text-content placeholder:text-muted/60 focus:border-accent scroll-thin max-h-28 min-h-[38px] flex-1 resize-none rounded-lg border px-3 py-2 text-sm focus:outline-none"
        />
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
