'use client'

import { useEffect, useRef, useState } from 'react'
import { Section } from './Section'
import { Button } from '@/components/ui/Button'
import { useAsyncAction } from '@/hooks/useAsyncAction'
import { JarvisEvent, onJarvisEvent, startRecording, stopRecording } from '@/lib/tauri'
import type { Recording } from '@/types'

export function MicSection() {
  const [isRecording, setIsRecording] = useState(false)
  const [level, setLevel] = useState(0)
  const [recording, setRecording] = useState<Recording | null>(null)
  const { isBusy, error, run } = useAsyncAction()

  useMicLevel(setLevel)
  useStopOnUnmount(isRecording)

  async function toggle() {
    await run(async () => {
      if (isRecording) {
        setRecording(await stopRecording())
        setIsRecording(false)
        setLevel(0)
        return
      }

      await startRecording()
      setIsRecording(true)
    })
  }

  return (
    <Section
      title="Microfone"
      hint="Grava do microfone padrão e salva um WAV mono de 16 bits — o formato que a transcrição vai consumir. Ainda não transcreve."
      error={error}
    >
      <div className="flex items-center gap-2">
        <Button
          variant={isRecording ? 'subtle' : 'primary'}
          onClick={() => void toggle()}
          disabled={isBusy}
        >
          {isRecording ? 'Parar' : 'Gravar'}
        </Button>
        <LevelMeter level={level} isActive={isRecording} />
      </div>

      {recording ? (
        <dl className="text-muted grid grid-cols-[auto_1fr] gap-x-2 text-[11px]">
          <dt>Duração</dt>
          <dd className="text-content">{recording.durationSeconds.toFixed(1)}s</dd>
          <dt>Taxa</dt>
          <dd className="text-content">{recording.sampleRate} Hz</dd>
          <dt>Arquivo</dt>
          <dd className="text-content break-all">{recording.path}</dd>
        </dl>
      ) : null}
    </Section>
  )
}

/**
 * O pico bruto de fala normal fica lá embaixo na escala linear, e a barra mal sairia
 * do lugar. A raiz quadrada aproxima a percepção de volume e deixa o medidor útil.
 */
function LevelMeter({ level, isActive }: { level: number; isActive: boolean }) {
  const width = isActive ? Math.min(100, Math.sqrt(level) * 100) : 0

  return (
    <div
      role="meter"
      aria-label="Nível do microfone"
      aria-valuenow={Math.round(width)}
      aria-valuemin={0}
      aria-valuemax={100}
      className="bg-base border-border-soft h-2 flex-1 overflow-hidden rounded-full border"
    >
      <div
        className="bg-accent hud-glow h-full rounded-full transition-[width] duration-75"
        style={{ width: `${width}%` }}
      />
    </div>
  )
}

function useMicLevel(onLevel: (level: number) => void) {
  useEffect(() => {
    let unlisten: (() => void) | null = null
    let cancelled = false

    void onJarvisEvent<number>(JarvisEvent.MicLevel, onLevel).then((stop) => {
      // A gaveta pode fechar antes de o `listen` resolver; sem isso, o listener
      // ficaria pendurado para sempre.
      if (cancelled) stop()
      else unlisten = stop
    })

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [onLevel])
}

/** Fechar a gaveta no meio de uma gravação deixaria o microfone aberto no backend. */
function useStopOnUnmount(isRecording: boolean) {
  // Ref, e não a prop direto: a limpeza roda uma vez só (deps vazias) e precisa
  // enxergar o estado do momento do unmount, não o da primeira renderização.
  const recordingRef = useRef(isRecording)

  useEffect(() => {
    recordingRef.current = isRecording
  }, [isRecording])

  useEffect(
    () => () => {
      if (recordingRef.current) void stopRecording().catch(() => undefined)
    },
    [],
  )
}
