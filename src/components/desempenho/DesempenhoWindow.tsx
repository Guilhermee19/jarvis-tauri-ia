'use client'

import { useState } from 'react'
import { DesempenhoPanel } from './DesempenhoPanel'
import { FloatingPanel, type PanelPosition, type PanelSize } from '@/components/ui/FloatingPanel'
import { useDesempenhoStore, useJanelaStore, zDaJanela } from '@/stores'

/**
 * Janelinha flutuante, no mesmo molde da conversa e da casa.
 *
 * Flutuante e não gaveta pelo mesmo motivo da casa, e aqui ainda mais forte: o uso disto
 * é deixar aberta num canto ENQUANTO outra coisa acontece — ver a CPU subir na hora em
 * que o Whisper transcreve é a única forma de ligar uma coisa à outra.
 */
export function DesempenhoWindow() {
  const abertas = useJanelaStore((state) => state.abertas)
  const abrir = useJanelaStore((state) => state.abrir)
  const fechar = useJanelaStore((state) => state.fechar)
  const isOpen = abertas.includes('desempenho')
  // Fora do `FloatingPanel` porque ele some do DOM ao fechar, e a janelinha precisa
  // reabrir onde e do tamanho que você a deixou.
  const [position, setPosition] = useState<PanelPosition | null>(null)
  const [size, setSize] = useState<PanelSize | null>(null)
  const [maximized, setMaximized] = useState(false)

  return (
    <FloatingPanel
      open={isOpen}
      onClose={() => fechar('desempenho')}
      zIndex={zDaJanela(abertas, 'desempenho')}
      onFocus={() => abrir('desempenho')}
      position={position}
      onPositionChange={setPosition}
      size={size}
      onSizeChange={setSize}
      maximized={maximized}
      onMaximizedChange={setMaximized}
      title="Desempenho"
      description="Processador, memória e placa de vídeo, com o último minuto de história."
      actions={<Resumo />}
    >
      <DesempenhoPanel />
    </FloatingPanel>
  )
}

/**
 * O uso de processador no cabeçalho.
 *
 * Sobrevive à rolagem, e é o número que se quer de relance com a janela pequena num
 * canto — no molde da contagem de aparelhos da casa.
 */
function Resumo() {
  const cpu = useDesempenhoStore((state) => state.atual?.cpu)

  if (cpu === undefined) return null

  return (
    <span className="text-muted shrink-0 text-[10px] tracking-[0.14em] uppercase tabular-nums">
      cpu {Math.round(cpu)}%
    </span>
  )
}
