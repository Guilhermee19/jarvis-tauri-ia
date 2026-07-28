'use client'

import { DiagnosticsPanel } from '@/components/diagnostics/DiagnosticsPanel'
import { Sheet, SheetContent } from '@/components/ui/Sheet'
import { useSheetStore } from '@/stores'

export function DiagnosticsSheet() {
  const isOpen = useSheetStore((state) => state.activeSheet === 'diagnostics')
  const open = useSheetStore((state) => state.open)
  const close = useSheetStore((state) => state.close)

  return (
    <Sheet
      modal={false}
      open={isOpen}
      onOpenChange={(next) => (next ? open('diagnostics') : close())}
    >
      {/* Fechar a gaveta desmonta as seções, e é o `useEffect` de limpeza delas que
          solta microfone e câmera — nenhum sensor sobrevive à gaveta fechada. */}
      <SheetContent
        title="Diagnóstico"
        description="Testes isolados de microfone, voz, webcam e captura de tela."
      >
        <DiagnosticsPanel />
      </SheetContent>
    </Sheet>
  )
}
