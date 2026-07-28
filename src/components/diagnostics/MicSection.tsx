'use client'

import { Section } from './Section'
import { Button } from '@/components/ui/Button'
import { useSensorStore } from '@/stores'

export function MicSection() {
  const isOn = useSensorStore((state) => state.isMicOn)
  const isBusy = useSensorStore((state) => state.isMicBusy)
  const level = useSensorStore((state) => state.micLevel)
  const error = useSensorStore((state) => state.micError)
  const recording = useSensorStore((state) => state.lastRecording)
  const toggleMic = useSensorStore((state) => state.toggleMic)

  return (
    <Section
      title="Microfone"
      hint="Mesmo interruptor do ícone na barra: grava do microfone padrão e salva um WAV mono de 16 bits, o formato que a transcrição vai consumir. Ainda não transcreve."
      error={error}
    >
      <div className="flex items-center gap-2">
        <Button
          variant={isOn ? 'subtle' : 'primary'}
          onClick={() => void toggleMic()}
          disabled={isBusy}
        >
          {isOn ? 'Parar' : 'Gravar'}
        </Button>
        <LevelMeter level={level} isActive={isOn} />
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
