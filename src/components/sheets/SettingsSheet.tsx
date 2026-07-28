'use client'

import { SettingsForm } from '@/components/settings/SettingsForm'
import { Sheet, SheetContent } from '@/components/ui/Sheet'
import { useSettingsStore, useSheetStore } from '@/stores'
import type { AppSettings } from '@/types'

export function SettingsSheet() {
  const isOpen = useSheetStore((state) => state.activeSheet === 'settings')
  const open = useSheetStore((state) => state.open)
  const close = useSheetStore((state) => state.close)

  const settings = useSettingsStore((state) => state.settings)
  const isSaving = useSettingsStore((state) => state.isSaving)
  const error = useSettingsStore((state) => state.error)
  const save = useSettingsStore((state) => state.save)

  async function onSubmit(next: AppSettings) {
    if (await save(next)) close()
  }

  return (
    <Sheet modal={false} open={isOpen} onOpenChange={(next) => (next ? open('settings') : close())}>
      <SheetContent title="Configurações" description="Chave da API e nome do assistente.">
        <div className="scroll-thin flex-1 overflow-y-auto p-3">
          {error ? (
            <p className="bg-danger/10 text-danger mb-3 rounded-lg px-3 py-2 text-[11px]">
              {error}
            </p>
          ) : null}

          {/* O Sheet desmonta o conteúdo ao fechar, então o form já nasce com os
              valores atuais a cada abertura — sem efeito de sincronização. */}
          <SettingsForm
            initial={settings}
            isSaving={isSaving}
            onSubmit={(next) => void onSubmit(next)}
            onCancel={close}
          />
        </div>
      </SheetContent>
    </Sheet>
  )
}
