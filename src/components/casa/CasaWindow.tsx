'use client'

import { useState } from 'react'
import { CasaPanel } from './CasaPanel'
import { FloatingPanel, type PanelPosition, type PanelSize } from '@/components/ui/FloatingPanel'
import { useCasaStore, useJanelaStore, zDaJanela } from '@/stores'

/**
 * Janelinha flutuante, como a da conversa — e não gaveta como o Diagnóstico.
 *
 * A diferença não é de gosto: gaveta é para o que se lê e se fecha, e a lista da casa é
 * para ficar aberta enquanto você mexe em outra coisa. Arrastar para o canto e continuar
 * conversando é o uso normal dela.
 */
export function CasaWindow() {
  const abertas = useJanelaStore((state) => state.abertas)
  const abrir = useJanelaStore((state) => state.abrir)
  const fechar = useJanelaStore((state) => state.fechar)
  const isOpen = abertas.includes('casa')
  // Fora do `FloatingPanel` pelo mesmo motivo do chat: ele some do DOM ao fechar, e a
  // janelinha precisa reabrir onde e do tamanho que você a deixou.
  const [position, setPosition] = useState<PanelPosition | null>(null)
  const [size, setSize] = useState<PanelSize | null>(null)
  const [maximized, setMaximized] = useState(false)

  return (
    <FloatingPanel
      open={isOpen}
      onClose={() => fechar('casa')}
      zIndex={zDaJanela(abertas, 'casa')}
      onFocus={() => abrir('casa')}
      position={position}
      onPositionChange={setPosition}
      size={size}
      onSizeChange={setSize}
      maximized={maximized}
      onMaximizedChange={setMaximized}
      title="Casa"
      description="Aparelhos inteligentes encontrados na sua rede local."
      actions={<Contagem />}
    >
      <CasaPanel />
    </FloatingPanel>
  )
}

/**
 * Quantos aparelhos, no cabeçalho.
 *
 * Fica aqui e não no corpo porque sobrevive à rolagem: com a lista grande, saber que são
 * sete sem voltar ao topo é a informação que se quer de relance.
 */
function Contagem() {
  const total = useCasaStore((state) => state.aparelhos.length)

  if (total === 0) return null

  return (
    <span className="text-muted shrink-0 text-[10px] tracking-[0.14em] uppercase tabular-nums">
      {total} {total === 1 ? 'aparelho' : 'aparelhos'}
    </span>
  )
}
