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
  const { isRecording, isTranscribing, start, stop, level, error, clearError } = useVoiceInput()

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
    // O erro anterior sai da tela ao tentar de novo, e não ao chegar o próximo: se
    // ficasse, um "não ouvi nada" de dois minutos atrás continuaria acusando o
    // microfone enquanto a gravação nova corre.
    clearError()
    await start()
  }

  return (
    <div className="border-border-soft bg-surface/70 border-t px-3 py-3 backdrop-blur-sm">
      {error || isRecording ? (
        <div className="mx-auto mb-2 flex w-full max-w-[560px] flex-col gap-1.5">
          {error ? <VoiceError message={error} onDismiss={clearError} /> : null}
          {isRecording ? <LevelBar level={level} /> : null}
        </div>
      ) : null}

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
          {/* O tamanho é obrigatório: sem `h-*`/`w-*` o SVG não tem como se medir e o
              botão sai vazio. Mesma medida do microfone da barra de ícones. */}
          {isTranscribing ? (
            <Spinner className="h-4.5 w-4.5" />
          ) : (
            <MicIcon className="h-4.5 w-4.5" />
          )}
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

/**
 * Enquanto o Whisper trabalha o botão fica desabilitado, e um microfone parado é
 * indistinguível de um botão que não respondeu ao clique. O giro é a diferença entre
 * "está pensando" e "não funcionou".
 */
function Spinner({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="1em"
      height="1em"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      className={cn('animate-spin', className)}
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="9" className="opacity-25" />
      <path d="M21 12a9 9 0 0 0-9-9" />
    </svg>
  )
}

/**
 * O erro do ditado, ao lado do botão que o causou.
 *
 * Antes ele ia para o alerta do HUD da home — que fica ATRÁS do painel de chat. Na
 * prática o botão falhava em silêncio: clicar, falar, clicar de novo e nada. As
 * mensagens do Rust já dizem o que fazer ("baixe o whisper-blas-bin-x64.zip…",
 * "Configurações › Privacidade › Microfone"), só não tinham onde aparecer.
 */
function VoiceError({ message, onDismiss }: { message: string; onDismiss: () => void }) {
  return (
    <div
      role="alert"
      className="border-danger/30 bg-danger/10 text-danger flex items-start gap-2 rounded border px-2 py-1.5 text-[11px] leading-relaxed"
    >
      <span className="flex-1 whitespace-pre-line">{message}</span>
      <button
        type="button"
        onClick={onDismiss}
        aria-label="Dispensar o aviso do microfone"
        className="text-danger/70 hover:text-danger shrink-0 leading-none"
      >
        ✕
      </button>
    </div>
  )
}

/**
 * Prova visual de que o microfone está captando, enquanto ainda dá para agir.
 *
 * Um mic mudo no painel do Windows abre sem erro nenhum e grava silêncio — o app só
 * descobria isso segundos depois, no "não ouvi nada" do Whisper. Com a barra parada
 * em zero a resposta chega no instante em que o usuário começa a falar.
 *
 * A raiz quadrada é a mesma do medidor da bancada: o pico de fala normal fica lá
 * embaixo na escala linear e a barra mal sairia do lugar.
 */
function LevelBar({ level }: { level: number }) {
  const width = Math.min(100, Math.sqrt(level) * 100)

  return (
    <div className="flex items-center gap-2">
      <span className="text-muted shrink-0 text-[10px] tracking-[0.14em] uppercase">Ouvindo</span>
      <div
        role="meter"
        aria-label="Nível do microfone"
        aria-valuenow={Math.round(width)}
        aria-valuemin={0}
        aria-valuemax={100}
        className="bg-base border-border-soft h-1.5 flex-1 overflow-hidden rounded-full border"
      >
        <div
          className="bg-accent hud-glow h-full rounded-full transition-[width] duration-75"
          style={{ width: `${width}%` }}
        />
      </div>
    </div>
  )
}
