'use client'

import { CotacoesPanel } from './CotacoesPanel'
import { FloatingPanel } from '@/components/ui/FloatingPanel'
import { useCotacoesStore, useJanelaStore, zDaJanela } from '@/stores'

/**
 * Janelinha flutuante, no mesmo molde do desempenho e da casa.
 *
 * Abre sozinha quando ele pergunta uma cotação (o `AcaoDeUi::Cotacoes` chega no
 * `useSensorEvents`), e também pelo ícone da barra — porque depois de ter perguntado uma
 * vez, reabrir para conferir o número não devia exigir perguntar de novo.
 */
export function CotacoesWindow() {
  const abertas = useJanelaStore((state) => state.abertas)
  const abrir = useJanelaStore((state) => state.abrir)
  const fechar = useJanelaStore((state) => state.fechar)
  const isOpen = abertas.includes('cotacoes')
  const arranjo = useJanelaStore((state) => state.arranjos.cotacoes)
  const ajustar = useJanelaStore((state) => state.ajustar)
  const fixadas = useJanelaStore((state) => state.fixadas)
  const fixar = useJanelaStore((state) => state.fixar)

  return (
    <FloatingPanel
      open={isOpen}
      onClose={() => fechar('cotacoes')}
      zIndex={zDaJanela(abertas, 'cotacoes')}
      onFocus={() => abrir('cotacoes')}
      position={arranjo?.posicao ?? null}
      onPositionChange={(posicao) => ajustar('cotacoes', { posicao })}
      size={arranjo?.tamanho ?? null}
      onSizeChange={(tamanho) => ajustar('cotacoes', { tamanho })}
      maximized={arranjo?.maximizada ?? false}
      onMaximizedChange={(maximizada) => ajustar('cotacoes', { maximizada })}
      fixada={fixadas.includes('cotacoes')}
      onFixadaChange={(fixada) => fixar('cotacoes', fixada)}
      title="Cotações"
      description="Dólar, euro, bitcoin e ethereum, com a variação do dia."
      actions={<Quantas />}
    >
      <CotacoesPanel />
    </FloatingPanel>
  )
}

/** Quantas moedas o card está mostrando, no cabeçalho — igual à contagem de abas do navegador. */
function Quantas() {
  const total = useCotacoesStore((state) => state.cotacoes.length)

  if (total === 0) return null

  return (
    <span className="text-muted shrink-0 text-[10px] tracking-[0.14em] uppercase tabular-nums">
      {total} moedas
    </span>
  )
}
