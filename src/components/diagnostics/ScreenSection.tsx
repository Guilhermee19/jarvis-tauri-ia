'use client'

import { useEffect, useState } from 'react'
import { Preview, Section } from './Section'
import { Button } from '@/components/ui/Button'
import { useAsyncAction } from '@/hooks/useAsyncAction'
import { captureScreenshot, listMonitors } from '@/lib/tauri'
import type { CapturedImage, MonitorInfo } from '@/types'

export function ScreenSection() {
  const [monitors, setMonitors] = useState<MonitorInfo[]>([])
  const [selected, setSelected] = useState<number | null>(null)
  const [shot, setShot] = useState<CapturedImage | null>(null)
  const { isBusy, error, run, setError } = useAsyncAction()

  useEffect(() => {
    listMonitors()
      .then(setMonitors)
      .catch((cause: unknown) => setError(cause instanceof Error ? cause.message : String(cause)))
  }, [setError])

  return (
    <Section
      title="Tela"
      hint="Captura a tela em PNG — sem compressão com perda, porque é texto de interface que o modelo vai precisar ler depois."
      error={error}
    >
      {/* O seletor só faz sentido com mais de uma tela; com uma só, o nome dela já
          diz tudo que o usuário precisa conferir. */}
      {monitors.length > 1 ? (
        <label className="flex flex-col gap-1">
          <span className="text-muted text-[10px] tracking-[0.14em] uppercase">Monitor</span>
          <select
            value={selected ?? ''}
            onChange={(event) =>
              setSelected(event.target.value ? Number(event.target.value) : null)
            }
            className="border-border-soft bg-base text-content focus:border-accent rounded-lg border px-2 py-1.5 text-sm focus:outline-none"
          >
            <option value="">Principal</option>
            {monitors.map((monitor) => (
              <option key={monitor.id} value={monitor.id}>
                {monitor.name} · {monitor.width}×{monitor.height}
                {monitor.isPrimary ? ' (principal)' : ''}
              </option>
            ))}
          </select>
        </label>
      ) : (
        <p className="text-muted text-[11px]">
          {monitors[0]
            ? `Monitor único: ${monitors[0].name} · ${monitors[0].width}×${monitors[0].height}`
            : 'Nenhum monitor detectado ainda.'}
        </p>
      )}

      <Button
        onClick={() =>
          void run(async () => setShot(await captureScreenshot(selected ?? undefined)))
        }
        disabled={isBusy}
        className="self-start"
      >
        Capturar tela
      </Button>

      {shot ? (
        <div className="flex flex-col gap-1">
          <span className="text-muted text-[10px] tracking-[0.14em] uppercase">
            {shot.width}×{shot.height}
          </span>
          <Preview src={shot.dataUrl} label="Captura da tela" />
        </div>
      ) : null}
    </Section>
  )
}
