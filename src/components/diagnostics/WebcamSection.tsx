'use client'

import { useState } from 'react'
import { Preview, Section } from './Section'
import { Button } from '@/components/ui/Button'
import { useAsyncAction } from '@/hooks/useAsyncAction'
import { captureWebcamFrame } from '@/lib/tauri'
import { useSensorStore } from '@/stores'
import type { CapturedImage } from '@/types'

export function WebcamSection() {
  const isOn = useSensorStore((state) => state.isWebcamOn)
  const isBusy = useSensorStore((state) => state.isWebcamBusy)
  const frame = useSensorStore((state) => state.webcamFrame)
  const sensorError = useSensorStore((state) => state.webcamError)
  const toggleWebcam = useSensorStore((state) => state.toggleWebcam)

  const [captured, setCaptured] = useState<CapturedImage | null>(null)
  const { isBusy: isCapturing, error: captureError, run } = useAsyncAction()

  return (
    <Section
      title="Webcam"
      hint="Mesmo interruptor do ícone na barra — ligada, a imagem também vira o fundo da tela inicial. Capturar frame congela a imagem atual, sem nenhum reconhecimento."
      error={sensorError ?? captureError}
    >
      <div className="flex flex-wrap items-center gap-2">
        <Button
          variant={isOn ? 'subtle' : 'primary'}
          onClick={() => void toggleWebcam()}
          disabled={isBusy}
        >
          {isOn ? 'Fechar webcam' : 'Abrir webcam'}
        </Button>
        <Button
          variant="subtle"
          onClick={() => void run(async () => setCaptured(await captureWebcamFrame()))}
          disabled={isCapturing}
        >
          Capturar frame
        </Button>
      </div>

      {/* O quadro vem do store: existe UM laço de captura para a home e para cá. */}
      {frame ? <Preview src={frame} label="Preview da webcam" /> : null}

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
