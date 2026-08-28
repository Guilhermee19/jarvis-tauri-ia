'use client'

import { useState } from 'react'
import { DiagnosticsPanel } from '@/components/diagnostics/DiagnosticsPanel'
import { SettingsForm } from '@/components/settings/SettingsForm'
import { Sheet, SheetContent } from '@/components/ui/Sheet'
import { useJanelaStore, useSettingsStore } from '@/stores'
import type { AppSettings } from '@/types'

export function SettingsSheet() {
  const [tab, setTab] = useState<'settings' | 'diagnostics'>('settings')
  const isOpen = useJanelaStore((state) => state.gaveta === 'settings')
  const abrirGaveta = useJanelaStore((state) => state.abrirGaveta)
  const close = useJanelaStore((state) => state.fecharGaveta)

  const settings = useSettingsStore((state) => state.settings)
  const isSaving = useSettingsStore((state) => state.isSaving)
  const error = useSettingsStore((state) => state.error)
  const save = useSettingsStore((state) => state.save)

  async function onSubmit(next: AppSettings) {
    if (await save(next)) close()
  }

  return (
    <Sheet
      modal={false}
      open={isOpen}
      onOpenChange={(next) => (next ? abrirGaveta('settings') : close())}
    >
      <SheetContent title="Configurações" description="Chave da API e nome do assistente.">
        <div className="border-border-soft flex shrink-0 border-b px-3 pt-2">
          <button
            type="button"
            onClick={() => setTab('settings')}
            aria-selected={tab === 'settings'}
            className={
              tab === 'settings'
                ? 'border-accent text-accent border-b-2 px-3 py-2 text-xs'
                : 'text-muted border-b-2 border-transparent px-3 py-2 text-xs'
            }
          >
            Configurações
          </button>
          <button
            type="button"
            onClick={() => setTab('diagnostics')}
            aria-selected={tab === 'diagnostics'}
            className={
              tab === 'diagnostics'
                ? 'border-accent text-accent border-b-2 px-3 py-2 text-xs'
                : 'text-muted border-b-2 border-transparent px-3 py-2 text-xs'
            }
          >
            Diagnóstico
          </button>
        </div>

        {tab === 'diagnostics' ? (
          <div className="scroll-thin flex-1 overflow-y-auto">
            <DiagnosticsPanel />
          </div>
        ) : (
          <div className="scroll-thin flex-1 overflow-y-auto p-3">
            {error ? (
              <p className="bg-danger/10 text-danger mb-3 rounded-lg px-3 py-2 text-[11px]">
                {error}
              </p>
            ) : null}

            <SettingsForm
              initial={settings}
              isSaving={isSaving}
              onSubmit={(next) => void onSubmit(next)}
              onCancel={close}
            />
          </div>
        )}
      </SheetContent>
    </Sheet>
  )
}
