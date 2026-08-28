'use client'

import { useEffect } from 'react'

import { cn } from '@/lib/utils'
import { useDesempenhoStore } from '@/stores'

/**
 * Processador, memória e placa de vídeo, com um minuto de história.
 *
 * **O gráfico existe porque um número sozinho não responde à pergunta que se faz aqui.**
 * "A CPU está em 60%" não diz se ela subiu agora por causa do que você acabou de mandar,
 * ou se está assim há um minuto — e é justamente essa a diferença que se quer ver ao
 * abrir isto enquanto o Jarvis transcreve, pensa ou fala.
 *
 * A escala do gráfico é sempre 0–100 e nunca se ajusta ao maior valor visto: escala
 * automática faz um pico de 4% parecer igual a um de 90%, que é o oposto de acompanhar.
 */
export function DesempenhoPanel() {
  const atual = useDesempenhoStore((state) => state.atual)
  const cpu = useDesempenhoStore((state) => state.cpu)
  const gpu = useDesempenhoStore((state) => state.gpu)
  const memoria = useDesempenhoStore((state) => state.memoria)
  const erro = useDesempenhoStore((state) => state.erro)
  const iniciar = useDesempenhoStore((state) => state.iniciar)
  const parar = useDesempenhoStore((state) => state.parar)

  // O laço vive enquanto o painel estiver aberto. Amostrar uma tela que ninguém está
  // vendo é gastar processador para medir processador.
  useEffect(() => {
    iniciar()

    return parar
  }, [iniciar, parar])

  if (erro) {
    return (
      <div className="flex min-h-0 flex-1 flex-col gap-3 px-3 py-3">
        <p
          role="alert"
          className="border-danger/30 bg-danger/10 text-danger rounded-md border px-2.5 py-2 text-[11px] leading-relaxed"
        >
          {erro}
        </p>
      </div>
    )
  }

  return (
    <div className="scroll-thin flex min-h-0 flex-1 flex-col gap-2.5 overflow-y-auto px-3 py-3">
      <Medida
        titulo="Processador"
        valor={atual?.cpu ?? null}
        historico={cpu}
        detalhe={atual ? 'uso total, somando todos os núcleos' : 'medindo…'}
      />

      <Medida
        titulo="Memória"
        valor={memoria.at(-1) ?? null}
        historico={memoria}
        detalhe={
          atual && atual.memoriaTotal > 0
            ? `${bytes(atual.memoriaUsada)} de ${bytes(atual.memoriaTotal)}`
            : 'medindo…'
        }
      />

      <Medida
        titulo="Placa de vídeo"
        valor={atual?.gpu ?? null}
        historico={gpu}
        detalhe={
          atual
            ? [
                atual.gpuNome || 'placa não identificada',
                // Zero aqui quer dizer "sem contador", e não "sem uso": vídeo integrado
                // costuma não expor memória dedicada, e uma linha "0 de 0" seria mentira.
                atual.gpuMemoriaTotal > 0
                  ? `${bytes(atual.gpuMemoriaUsada)} de ${bytes(atual.gpuMemoriaTotal)}`
                  : null,
              ]
                .filter(Boolean)
                .join(' · ')
            : 'medindo…'
        }
      />

      <p className="text-muted/70 mt-0.5 text-[10px] leading-relaxed">
        Uma leitura por segundo, e o gráfico guarda o último minuto. É o mesmo intervalo do
        Gerenciador de Tarefas: os contadores medem a distância entre duas leituras, então ler mais
        rápido não traz mais informação — traz a mesma com mais ruído.
      </p>
    </div>
  )
}

function Medida({
  titulo,
  valor,
  historico,
  detalhe,
}: {
  titulo: string
  valor: number | null
  historico: number[]
  detalhe: string
}) {
  return (
    <div className="border-border-soft bg-base/40 rounded-md border px-3 py-2.5">
      <div className="flex items-baseline gap-2">
        <span className="text-content flex-1 text-xs">{titulo}</span>
        <span
          className={cn(
            'text-sm tabular-nums',
            // O acento entra só quando a coisa está de fato carregada: pintar 8% de
            // laranja faria a cor perder o significado quando ela importasse.
            valor !== null && valor >= 80
              ? 'text-danger'
              : valor !== null && valor >= 50
                ? 'text-accent'
                : 'text-muted',
          )}
        >
          {valor === null ? '—' : `${Math.round(valor)}%`}
        </span>
      </div>

      <Grafico serie={historico} />

      <p className="text-muted/70 mt-1.5 truncate text-[10px]">{detalhe}</p>
    </div>
  )
}

/**
 * A história recente, como uma área preenchida.
 *
 * SVG e não canvas: são 60 pontos redesenhados uma vez por segundo, o que o React resolve
 * sem esforço — e um canvas exigiria uma `ref`, um contexto e um redesenho manual para
 * entregar o mesmo desenho.
 *
 * `preserveAspectRatio="none"` porque o gráfico deve ESTICAR com a janela: a proporção do
 * traço não significa nada, a altura sim.
 */
function Grafico({ serie }: { serie: number[] }) {
  const LARGURA = 100
  const ALTURA = 28

  // Menos de dois pontos não é uma linha. Desenhar um ponto só daria um traço reto no
  // fundo do gráfico, que se lê como "esteve em zero esse tempo todo".
  if (serie.length < 2) {
    return <div className="bg-surface-hover mt-2 h-7 w-full rounded-sm" />
  }

  const passo = LARGURA / (serie.length - 1)
  const pontos = serie
    .map(
      (valor, indice) =>
        `${(indice * passo).toFixed(2)},${(ALTURA - (valor / 100) * ALTURA).toFixed(2)}`,
    )
    .join(' ')

  return (
    <svg
      viewBox={`0 0 ${LARGURA} ${ALTURA}`}
      preserveAspectRatio="none"
      className="bg-surface-hover mt-2 h-7 w-full rounded-sm"
      aria-hidden="true"
    >
      {/* A área primeiro, a linha por cima: a linha sozinha some no fundo escuro do
          painel, e a área sozinha não mostra a variação fina. */}
      <polygon points={`0,${ALTURA} ${pontos} ${LARGURA},${ALTURA}`} className="fill-accent/20" />
      <polyline
        points={pontos}
        fill="none"
        className="stroke-accent"
        strokeWidth={1}
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  )
}

/**
 * Bytes em unidade legível.
 *
 * Base 1024 e sufixo "GB" porque é o que o Windows mostra: escrever "GiB" seria mais
 * correto e faria o número não bater com o Gerenciador de Tarefas ao lado.
 */
function bytes(valor: number): string {
  const gb = valor / 1024 ** 3

  if (gb >= 1) return `${gb.toFixed(1).replace('.', ',')} GB`

  return `${Math.round(valor / 1024 ** 2)} MB`
}
