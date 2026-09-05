'use client'

import { TempoPanel } from './TempoPanel'
import { FloatingPanel } from '@/components/ui/FloatingPanel'
import { useJanelaStore, useTempoStore, zDaJanela } from '@/stores'

/**
 * Janelinha flutuante, no mesmo molde das cotações.
 *
 * Abre sozinha quando ele pergunta do tempo (o `AcaoDeUi::Tempo` chega no
 * `useSensorEvents`), e continua aberta depois — conferir a semana de novo não devia
 * exigir perguntar de novo, que é o mesmo argumento do card de cotações.
 */
export function TempoWindow() {
  const abertas = useJanelaStore((state) => state.abertas)
  const abrir = useJanelaStore((state) => state.abrir)
  const fechar = useJanelaStore((state) => state.fechar)
  const isOpen = abertas.includes('tempo')
  const arranjo = useJanelaStore((state) => state.arranjos.tempo)
  const ajustar = useJanelaStore((state) => state.ajustar)
  const fixadas = useJanelaStore((state) => state.fixadas)
  const fixar = useJanelaStore((state) => state.fixar)

  return (
    <FloatingPanel
      open={isOpen}
      onClose={() => fechar('tempo')}
      zIndex={zDaJanela(abertas, 'tempo')}
      onFocus={() => abrir('tempo')}
      position={arranjo?.posicao ?? null}
      onPositionChange={(posicao) => ajustar('tempo', { posicao })}
      size={arranjo?.tamanho ?? null}
      onSizeChange={(tamanho) => ajustar('tempo', { tamanho })}
      maximized={arranjo?.maximizada ?? false}
      onMaximizedChange={(maximizada) => ajustar('tempo', { maximizada })}
      fixada={fixadas.includes('tempo')}
      onFixadaChange={(fixada) => fixar('tempo', fixada)}
      title="Tempo"
      description="A previsão de hoje e dos próximos dias, pela Open-Meteo."
      actions={<Onde />}
    >
      <TempoPanel />
    </FloatingPanel>
  )
}

/** O lugar no cabeçalho — o mesmo papel da contagem de moedas no card de cotações. */
function Onde() {
  const lugar = useTempoStore((state) => state.lugar)
  const previsao = useTempoStore((state) => state.previsao)

  if (previsao === null) return null

  return (
    <span className="text-muted max-w-[140px] shrink-0 truncate text-[10px] tracking-[0.14em] uppercase">
      {lugar === '' ? 'Aqui' : lugar}
    </span>
  )
}
