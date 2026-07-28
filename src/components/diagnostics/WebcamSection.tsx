'use client'

import { useEffect, useState } from 'react'
import { Preview, Section } from './Section'
import { Button } from '@/components/ui/Button'
import { useAsyncAction } from '@/hooks/useAsyncAction'
import { captureWebcamFrame, closeWebcam, openWebcam } from '@/lib/tauri'
import type { CapturedImage } from '@/types'

/** ~11 quadros por segundo: suficiente para parecer vivo sem saturar o IPC com base64. */
const PREVIEW_INTERVAL_MS = 90

export function WebcamSection() {
  const [isOpen, setIsOpen] = useState(false)
  const [preview, setPreview] = useState<CapturedImage | null>(null)
  const [captured, setCaptured] = useState<CapturedImage | null>(null)
  const { isBusy, error, run, setError } = useAsyncAction()

  usePreviewLoop(isOpen, setPreview, setError)
  useCloseOnUnmount()

  async function toggle() {
    await run(async () => {
      if (isOpen) {
        await closeWebcam()
        setIsOpen(false)
        setPreview(null)
        return
      }

      await openWebcam()
      setIsOpen(true)
    })
  }

  return (
    <Section
      title="Webcam"
      hint="Abre a câmera e mostra o preview ao vivo. Capturar frame congela a imagem atual — sem nenhum reconhecimento nesta versão."
      error={error}
    >
      <div className="flex flex-wrap items-center gap-2">
        <Button
          variant={isOpen ? 'subtle' : 'primary'}
          onClick={() => void toggle()}
          disabled={isBusy}
        >
          {isOpen ? 'Fechar webcam' : 'Abrir webcam'}
        </Button>
        <Button
          variant="subtle"
          onClick={() => void run(async () => setCaptured(await captureWebcamFrame()))}
          disabled={isBusy}
        >
          Capturar frame
        </Button>
      </div>

      {preview ? <Preview src={preview.dataUrl} label="Preview da webcam" /> : null}

      {captured ? (
        <div className="flex flex-col gap-1">
          <span className="text-muted text-[10px] tracking-[0.14em] uppercase">
            Frame capturado · {captured.width}×{captured.height}
          </span>
          <Preview src={captured.dataUrl} label="Frame capturado da webcam" />
        </div>
      ) : null}
    </Section>
  )
}

/**
 * Laço com `setTimeout` encadeado, não `setInterval`: se um frame demorar mais que o
 * intervalo, os pedidos se empilhariam e a webcam nunca alcançaria a fila.
 */
function usePreviewLoop(
  isOpen: boolean,
  onFrame: (frame: CapturedImage) => void,
  onError: (message: string) => void,
) {
  useEffect(() => {
    if (!isOpen) return
    let active = true

    async function loop() {
      while (active) {
        try {
          const frame = await captureWebcamFrame()
          if (!active) return
          onFrame(frame)
        } catch (cause) {
          // Um erro no meio do preview (câmera arrancada da USB) para o laço em vez
          // de repetir a mesma falha 11 vezes por segundo.
          onError(cause instanceof Error ? cause.message : String(cause))
          return
        }
        await new Promise((resolve) => setTimeout(resolve, PREVIEW_INTERVAL_MS))
      }
    }

    void loop()
    return () => {
      active = false
    }
  }, [isOpen, onFrame, onError])
}

/** Fechar a gaveta com a câmera aberta deixaria a luz da webcam acesa. */
function useCloseOnUnmount() {
  useEffect(
    () => () => {
      void closeWebcam().catch(() => undefined)
    },
    [],
  )
}
