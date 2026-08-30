'use client'

import { CamerasPanel } from './CamerasPanel'
import { FloatingPanel } from '@/components/ui/FloatingPanel'
import { useCamerasStore, useJanelaStore, zDaJanela } from '@/stores'

/**
 * Janelinha flutuante, como a da casa.
 *
 * Gaveta seria errado aqui pela mesma razão da casa, e um pouco mais forte: olhar a
 * garagem enquanto se conversa é o uso normal, e uma gaveta fecharia a conversa para
 * mostrar a imagem.
 */
export function CamerasWindow() {
  const abertas = useJanelaStore((state) => state.abertas)
  const abrir = useJanelaStore((state) => state.abrir)
  const fechar = useJanelaStore((state) => state.fechar)
  const isOpen = abertas.includes('cameras')
  const arranjo = useJanelaStore((state) => state.arranjos.cameras)
  const ajustar = useJanelaStore((state) => state.ajustar)
  const fixadas = useJanelaStore((state) => state.fixadas)
  const fixar = useJanelaStore((state) => state.fixar)

  return (
    <FloatingPanel
      open={isOpen}
      onClose={() => fechar('cameras')}
      zIndex={zDaJanela(abertas, 'cameras')}
      onFocus={() => abrir('cameras')}
      position={arranjo?.posicao ?? null}
      onPositionChange={(posicao) => ajustar('cameras', { posicao })}
      size={arranjo?.tamanho ?? null}
      onSizeChange={(tamanho) => ajustar('cameras', { tamanho })}
      maximized={arranjo?.maximizada ?? false}
      onMaximizedChange={(maximizada) => ajustar('cameras', { maximizada })}
      fixada={fixadas.includes('cameras')}
      onFixadaChange={(fixada) => fixar('cameras', fixada)}
      title="Câmeras"
      description="As câmeras de segurança da sua rede local."
      actions={<Contagem />}
    >
      <CamerasPanel />
    </FloatingPanel>
  )
}

/**
 * Quantas câmeras, no cabeçalho.
 *
 * Conta as visíveis e não o catálogo inteiro: o número precisa bater com o que está na
 * grade, senão "3 câmeras" sobre duas imagens parece defeito.
 */
function Contagem() {
  const total = useCamerasStore((state) => state.cameras.filter((camera) => !camera.oculto).length)

  if (total === 0) return null

  return (
    <span className="text-muted shrink-0 text-[10px] tracking-[0.14em] uppercase tabular-nums">
      {total} {total === 1 ? 'câmera' : 'câmeras'}
    </span>
  )
}
