'use client'

import { DesempenhoPanel } from './DesempenhoPanel'
import { FloatingPanel } from '@/components/ui/FloatingPanel'
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
  // O arranjo mora no `janelaStore` porque agora ele sobrevive ao fechamento do APP,
  // e não só ao da janelinha — e porque as três janelas guardavam o mesmo trio de
  // estados, cada uma por conta própria.
  const arranjo = useJanelaStore((state) => state.arranjos.desempenho)
  const ajustar = useJanelaStore((state) => state.ajustar)
  const fixadas = useJanelaStore((state) => state.fixadas)
  const fixar = useJanelaStore((state) => state.fixar)

  return (
    <FloatingPanel
      open={isOpen}
      onClose={() => fechar('desempenho')}
      zIndex={zDaJanela(abertas, 'desempenho')}
      onFocus={() => abrir('desempenho')}
      position={arranjo?.posicao ?? null}
      onPositionChange={(posicao) => ajustar('desempenho', { posicao })}
      size={arranjo?.tamanho ?? null}
      onSizeChange={(tamanho) => ajustar('desempenho', { tamanho })}
      maximized={arranjo?.maximizada ?? false}
      onMaximizedChange={(maximizada) => ajustar('desempenho', { maximizada })}
      fixada={fixadas.includes('desempenho')}
      onFixadaChange={(fixada) => fixar('desempenho', fixada)}
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
