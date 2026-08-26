'use client'

import { useRef, useState, type KeyboardEvent } from 'react'
import { MicIcon } from '@/components/ui/icons'
import { Button } from '@/components/ui/Button'
import { useVoiceInput } from '@/hooks/useVoiceInput'
import { cn } from '@/lib/utils'

interface ChatInputProps {
  onSend: (content: string) => void
  disabled: boolean
}

export function ChatInput({ onSend, disabled }: ChatInputProps) {
  const [value, setValue] = useState('')
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const { isRecording, isTranscribing, start, stop } = useVoiceInput()

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

  /**
   * Alterna em vez de "segurar para falar": um botão que só funciona enquanto o
   * ponteiro está pressionado não tem como ser operado pelo teclado, e o ganho seria
   * só economizar um clique.
   */
  async function toggleMic() {
    if (isRecording) {
      const heard = await stop()
      // Preenche o campo, NÃO envia. O Whisper erra, e o que está do outro lado abre
      // programas — ler antes de mandar é barato.
      if (heard) setValue((current) => (current ? `${current} ${heard}` : heard))
      textareaRef.current?.focus()
      return
    }
    await start()
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
          placeholder={
            isRecording ? 'Ouvindo… clique no microfone para parar' : 'Fale com o Jarvis…'
          }
          className="border-border-soft bg-base text-content placeholder:text-muted/60 focus:border-accent scroll-thin max-h-28 min-h-[38px] flex-1 resize-none rounded-lg border px-3 py-2 text-sm focus:outline-none"
        />
        <Button
          variant={isRecording ? 'primary' : 'subtle'}
          onClick={() => void toggleMic()}
          disabled={isTranscribing}
          className={cn('h-[38px] px-2.5', isRecording && 'animate-pulse')}
          aria-pressed={isRecording}
          aria-label={isRecording ? 'Parar de gravar e transcrever' : 'Falar com o Jarvis'}
          title={isTranscribing ? 'Transcrevendo…' : 'Falar'}
        >
          <MicIcon />
        </Button>
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
