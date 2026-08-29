'use client'

import { useMemo } from 'react'
import ForceGraph2D from 'react-force-graph-2d'
import { useConhecimentoStore, useGrafoVisivel } from '@/stores'
import type { NoDoGrafo } from '@/types'

/**
 * O grafo desenhado, no estilo do Obsidian.
 *
 * Este arquivo é SÓ o desenho — filtro, busca e painel lateral moram no
 * `ConhecimentoPanel`. A separação existe porque o `ForceGraph2D` remonta a simulação
 * inteira quando o componente remonta, e os nós saem voando do zero: qualquer estado de
 * UI que mude junto o faria pular.
 *
 * A biblioteca muta os objetos que recebe (ela grava `x`, `y`, `vx`, `vy` em cada nó), e
 * por isso os dados são COPIADOS antes de entrar. Passar os objetos da store direto faria
 * o zustand carregar coordenadas de simulação dentro do estado.
 */
export function GrafoDoConhecimento({
  largura,
  altura,
}: {
  largura: number
  altura: number
}) {
  const grafo = useGrafoVisivel()
  const abrir = useConhecimentoStore((state) => state.abrir)
  const aberto = useConhecimentoStore((state) => state.aberto)

  // Só recalcula quando os IDs mudam. Sem isto, cada render entregaria objetos novos e a
  // simulação recomeçaria — o grafo ficaria tremendo para sempre.
  const assinatura = grafo.nos.map((no) => no.id).join('|')
  const dados = useMemo(
    () => ({
      nodes: grafo.nos.map((no) => ({ ...no })),
      links: grafo.arestas.map((aresta) => ({
        source: aresta.de,
        target: aresta.para,
        forca: aresta.forca,
        escrita: aresta.escrita,
      })),
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- a assinatura É a dependência
    [assinatura],
  )

  return (
    <ForceGraph2D
      width={largura}
      height={altura}
      graphData={dados}
      backgroundColor="transparent"
      // O HUD é escuro; a cor vem do tema, não fixada aqui.
      nodeRelSize={4}
      nodeVal={(no) => 1 + (no as NoDoGrafo).peso * 12}
      nodeLabel={(no) => (no as NoDoGrafo).rotulo}
      onNodeClick={(no) => void abrir(no as unknown as NoDoGrafo)}
      // Arestas escritas cheias, inferidas apagadas — a distinção é o ponto.
      linkColor={(link) => (linkEscrita(link) ? 'rgba(56,189,248,0.55)' : 'rgba(148,163,184,0.18)')}
      linkWidth={(link) => (linkEscrita(link) ? 1.6 : 0.6)}
      linkDirectionalParticles={(link) => (linkEscrita(link) ? 2 : 0)}
      linkDirectionalParticleWidth={1.6}
      linkDirectionalParticleSpeed={0.004}
      nodeCanvasObject={(no, ctx, escala) => desenharNo(no as NoDoGrafo, ctx, escala, aberto?.no.id)}
      nodeCanvasObjectMode={() => 'replace'}
      cooldownTicks={120}
      d3VelocityDecay={0.32}
    />
  )
}

function linkEscrita(link: unknown): boolean {
  return (link as { escrita?: boolean }).escrita === true
}

/**
 * Desenha um nó: círculo com brilho e o rótulo embaixo.
 *
 * Canvas próprio em vez do círculo padrão da biblioteca porque o padrão não tem brilho
 * nem rótulo legível, e é o brilho que faz isto parecer um HUD em vez de um diagrama.
 *
 * O rótulo some quando o zoom está longe: com dezenas de nós, os textos se sobrepõem e
 * viram uma mancha que esconde o próprio grafo.
 */
function desenharNo(
  no: NoDoGrafo & { x?: number; y?: number },
  ctx: CanvasRenderingContext2D,
  escala: number,
  selecionado?: string,
) {
  const { x = 0, y = 0 } = no
  const raio = 2 + no.peso * 7
  const destacado = no.id === selecionado

  const cor = COR_POR_TIPO[no.tipo] ?? COR_POR_TIPO.fato

  ctx.beginPath()
  ctx.arc(x, y, raio, 0, 2 * Math.PI)
  ctx.fillStyle = cor
  ctx.shadowColor = cor
  ctx.shadowBlur = destacado ? 24 : 10
  ctx.fill()
  ctx.shadowBlur = 0

  if (destacado) {
    ctx.beginPath()
    ctx.arc(x, y, raio + 3, 0, 2 * Math.PI)
    ctx.strokeStyle = cor
    ctx.lineWidth = 1 / escala
    ctx.stroke()
  }

  if (escala > 1.1) {
    ctx.font = `${11 / escala}px ui-sans-serif, system-ui, sans-serif`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'top'
    ctx.fillStyle = destacado ? cor : 'rgba(226,232,240,0.72)'
    ctx.fillText(no.rotulo, x, y + raio + 2 / escala)
  }
}

/**
 * Uma cor por tipo de nota.
 *
 * Valores literais, e não tokens do tema: isto é desenhado em `<canvas>`, onde não há CSS
 * — o `var(--color-accent)` não resolveria. São os mesmos tons da paleta do HUD.
 */
const COR_POR_TIPO: Record<string, string> = {
  fato: '#38bdf8',
  aprendido: '#4ade80',
  resumo: '#c084fc',
  rotina: '#fbbf24',
}
