'use client'

import { ConhecimentoPanel } from './ConhecimentoPanel'
import { FloatingPanel } from '@/components/ui/FloatingPanel'
import { useJanelaStore, zDaJanela } from '@/stores'

/**
 * Janelinha flutuante com o mapa do que o Jarvis sabe.
 *
 * A ideia veio do grafo do Obsidian, e faz sentido aqui pela mesma razão: a memória dele
 * É uma pasta de markdown com `[[links]]`. A diferença é que ninguém liga as notas à mão —
 * quem escreve é ele, e o que não estiver ligado o grafo infere por semelhança.
 */
export function ConhecimentoWindow() {
  const abertas = useJanelaStore((state) => state.abertas)
  const abrir = useJanelaStore((state) => state.abrir)
  const fechar = useJanelaStore((state) => state.fechar)
  const isOpen = abertas.includes('conhecimento')
  const arranjo = useJanelaStore((state) => state.arranjos.conhecimento)
  const ajustar = useJanelaStore((state) => state.ajustar)
  const fixadas = useJanelaStore((state) => state.fixadas)
  const fixar = useJanelaStore((state) => state.fixar)

  return (
    <FloatingPanel
      open={isOpen}
      onClose={() => fechar('conhecimento')}
      zIndex={zDaJanela(abertas, 'conhecimento')}
      onFocus={() => abrir('conhecimento')}
      position={arranjo?.posicao ?? null}
      onPositionChange={(posicao) => ajustar('conhecimento', { posicao })}
      size={arranjo?.tamanho ?? null}
      onSizeChange={(tamanho) => ajustar('conhecimento', { tamanho })}
      maximized={arranjo?.maximizada ?? false}
      onMaximizedChange={(maximizada) => ajustar('conhecimento', { maximizada })}
      fixada={fixadas.includes('conhecimento')}
      onFixadaChange={(fixada) => fixar('conhecimento', fixada)}
      title="Conhecimento"
      description="O mapa das notas que o Jarvis escreveu, e como elas se ligam."
    >
      <ConhecimentoPanel />
    </FloatingPanel>
  )
}
