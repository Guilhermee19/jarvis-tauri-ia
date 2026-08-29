'use client'

import { useState } from 'react'
import { NavegadorPanel } from './NavegadorPanel'
import { FloatingPanel } from '@/components/ui/FloatingPanel'
import { useJanelaStore, useNavegadorStore, zDaJanela } from '@/stores'

/**
 * Janelinha flutuante, no mesmo molde das outras — com uma diferença que importa.
 *
 * A página de dentro é uma camada NATIVA do sistema, acima de todo o HTML. Ela não é
 * coberta pelas outras janelinhas, e por isso o painel só a mostra quando está na frente.
 * Arrastar e redimensionar funcionam porque o `NavegadorPanel` remede o buraco a cada
 * mudança e reposiciona o webview — nada disso acontece por CSS.
 */
export function NavegadorWindow() {
  const abertas = useJanelaStore((state) => state.abertas)
  const abrir = useJanelaStore((state) => state.abrir)
  const fechar = useJanelaStore((state) => state.fechar)
  const isOpen = abertas.includes('navegador')
  const arranjo = useJanelaStore((state) => state.arranjos.navegador)
  const ajustar = useJanelaStore((state) => state.ajustar)
  const fixadas = useJanelaStore((state) => state.fixadas)
  const fixar = useJanelaStore((state) => state.fixar)

  // A página some enquanto a alça é arrastada. Sem isso o webview — camada nativa acima do
  // HTML — engoliria o ponteiro assim que ele entrasse na área dele, e o arrasto morreria
  // no meio do movimento.
  const [redimensionando, setRedimensionando] = useState(false)

  return (
    <FloatingPanel
      open={isOpen}
      onClose={() => fechar('navegador')}
      zIndex={zDaJanela(abertas, 'navegador')}
      onFocus={() => abrir('navegador')}
      position={arranjo?.posicao ?? null}
      onPositionChange={(posicao) => ajustar('navegador', { posicao })}
      size={arranjo?.tamanho ?? null}
      onSizeChange={(tamanho) => ajustar('navegador', { tamanho })}
      onResizingChange={setRedimensionando}
      maximized={arranjo?.maximizada ?? false}
      onMaximizedChange={(maximizada) => ajustar('navegador', { maximizada })}
      fixada={fixadas.includes('navegador')}
      onFixadaChange={(fixada) => fixar('navegador', fixada)}
      title="Navegador"
      description="Páginas abertas dentro do Jarvis, em abas."
      actions={<Contagem />}
    >
      <NavegadorPanel escondido={redimensionando} />
    </FloatingPanel>
  )
}

/** Quantas abas, no cabeçalho — sobrevive à rolagem da barra de linguetas. */
function Contagem() {
  const total = useNavegadorStore((state) => state.abas.length)

  if (total === 0) return null

  return (
    <span className="text-muted shrink-0 text-[10px] tracking-[0.14em] uppercase tabular-nums">
      {total} {total === 1 ? 'aba' : 'abas'}
    </span>
  )
}
