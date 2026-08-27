'use client'

import { DiagnosticsPanel } from '@/components/diagnostics/DiagnosticsPanel'
import { Sheet, SheetContent } from '@/components/ui/Sheet'
import { useJanelaStore } from '@/stores'

export function DiagnosticsSheet() {
  const isOpen = useJanelaStore((state) => state.gaveta === 'diagnostics')
  const abrirGaveta = useJanelaStore((state) => state.abrirGaveta)
  const fecharGaveta = useJanelaStore((state) => state.fecharGaveta)

  return (
    <Sheet
      modal={false}
      open={isOpen}
      onOpenChange={(next) => (next ? abrirGaveta('diagnostics') : fecharGaveta())}
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
