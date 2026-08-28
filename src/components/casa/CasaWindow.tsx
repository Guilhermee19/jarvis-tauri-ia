'use client'

import { CasaPanel } from './CasaPanel'
import { FloatingPanel } from '@/components/ui/FloatingPanel'
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
  // O arranjo mora no `janelaStore` porque agora ele sobrevive ao fechamento do APP,
  // e não só ao da janelinha — e porque as três janelas guardavam o mesmo trio de
  // estados, cada uma por conta própria.
  const arranjo = useJanelaStore((state) => state.arranjos.casa)
  const ajustar = useJanelaStore((state) => state.ajustar)
  const fixadas = useJanelaStore((state) => state.fixadas)
  const fixar = useJanelaStore((state) => state.fixar)

  return (
    <FloatingPanel
      open={isOpen}
      onClose={() => fechar('casa')}
      zIndex={zDaJanela(abertas, 'casa')}
      onFocus={() => abrir('casa')}
      position={arranjo?.posicao ?? null}
      onPositionChange={(posicao) => ajustar('casa', { posicao })}
      size={arranjo?.tamanho ?? null}
      onSizeChange={(tamanho) => ajustar('casa', { tamanho })}
      maximized={arranjo?.maximizada ?? false}
      onMaximizedChange={(maximizada) => ajustar('casa', { maximizada })}
      fixada={fixadas.includes('casa')}
      onFixadaChange={(fixada) => fixar('casa', fixada)}
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
