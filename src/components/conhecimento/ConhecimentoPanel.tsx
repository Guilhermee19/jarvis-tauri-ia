'use client'

import dynamic from 'next/dynamic'
import { useEffect, useRef, useState } from 'react'
import { Button } from '@/components/ui/Button'
import { SyncIcon } from '@/components/ui/icons'
import { cn } from '@/lib/utils'
import { useConhecimentoStore, useGrafoVisivel } from '@/stores'
import { TIPOS_DE_NOTA } from '@/types'

/**
 * Mede o espaço disponível, porque o `ForceGraph2D` exige largura e altura em NÚMERO.
 *
 * Mora aqui, e não junto do grafo, por uma razão que não é organização: qualquer import
 * ESTÁTICO do arquivo do grafo arrasta o `react-force-graph` para o bundle do servidor e
 * anula o `dynamic` logo abaixo. O sintoma é `ReferenceError: window is not defined` e a
 * página inteira em 500 — não só o grafo.
 */
function useTamanho() {
  const alvo = useRef<HTMLDivElement>(null)
  const [tamanho, setTamanho] = useState({ largura: 0, altura: 0 })

  useEffect(() => {
    const medir = () => {
      const caixa = alvo.current?.getBoundingClientRect()
      if (caixa) setTamanho({ largura: caixa.width, altura: caixa.height })
    }

    medir()
    const observador = new ResizeObserver(medir)
    if (alvo.current) observador.observe(alvo.current)

    return () => observador.disconnect()
  }, [])

  return { alvo, tamanho }
}

/**
 * O `ForceGraph2D` toca `window` no import — no Next com App Router isso quebra a
 * pré-renderização no servidor. `ssr: false` é obrigatório, não preferência.
 */
const Grafo = dynamic(
  () => import('./GrafoDoConhecimento').then((mod) => mod.GrafoDoConhecimento),
  { ssr: false },
)

export function ConhecimentoPanel() {
  const atualizar = useConhecimentoStore((state) => state.atualizar)
  const carregando = useConhecimentoStore((state) => state.carregando)
  const erro = useConhecimentoStore((state) => state.erro)
  const filtros = useConhecimentoStore((state) => state.filtros)
  const alternarFiltro = useConhecimentoStore((state) => state.alternarFiltro)
  const busca = useConhecimentoStore((state) => state.busca)
  const buscar = useConhecimentoStore((state) => state.buscar)
  const aberto = useConhecimentoStore((state) => state.aberto)
  const fechar = useConhecimentoStore((state) => state.fechar)

  const total = useConhecimentoStore((state) => state.grafo.nos.length)
  const visivel = useGrafoVisivel()
  const escritas = visivel.arestas.filter((aresta) => aresta.escrita).length

  // Carrega ao abrir a janelinha. As notas mudam a cada conversa, então um grafo montado
  // uma vez e guardado envelheceria sem ninguém perceber.
  useEffect(() => void atualizar(), [atualizar])

  const { alvo, tamanho } = useTamanho()

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {/* ---- contadores e busca ---- */}
      <div className="border-border-soft flex shrink-0 flex-wrap items-center gap-2 border-b px-3 py-2">
        <Medidor rotulo="memórias indexadas" valor={total} />
        <Medidor rotulo="nódulos ativos" valor={visivel.nos.length} />
        <Medidor rotulo="ligações" valor={visivel.arestas.length} detalhe={`${escritas} escritas`} />

        <input
          value={busca}
          onChange={(evento) => buscar(evento.target.value)}
          placeholder="buscar assunto…"
          spellCheck={false}
          aria-label="Buscar assunto"
          className="border-border-soft bg-base text-content placeholder:text-muted/60 focus:border-accent ml-auto min-w-0 flex-1 rounded border px-2 py-1 text-[11px] outline-none sm:max-w-48"
        />

        <Button
          variant="subtle"
          onClick={() => void atualizar()}
          disabled={carregando}
          aria-label="Atualizar o grafo"
          title="Reler as notas do disco"
        >
          <SyncIcon className={cn('h-3.5 w-3.5', carregando && 'animate-spin')} />
        </Button>
      </div>

      {/* ---- filtro por tipo ---- */}
      <div className="border-border-soft flex shrink-0 flex-wrap items-center gap-1.5 border-b px-3 py-1.5">
        {TIPOS_DE_NOTA.map((tipo) => {
          const ligado = filtros.length === 0 || filtros.includes(tipo.id)
          return (
            <button
              key={tipo.id}
              type="button"
              onClick={() => alternarFiltro(tipo.id)}
              aria-pressed={filtros.includes(tipo.id)}
              className={cn(
                'rounded-full border px-2 py-0.5 text-[10px] tracking-[0.1em] uppercase transition-colors',
                ligado
                  ? 'border-accent/40 bg-accent/10 text-accent'
                  : 'border-border-soft text-muted/60 hover:text-content',
              )}
            >
              {tipo.rotulo}
            </button>
          )
        })}
        <span className="text-muted/60 ml-auto text-[10px]">
          {filtros.length === 0 ? 'mostrando tudo' : `${filtros.length} de 4 tipos`}
        </span>
      </div>

      {erro ? (
        <p
          role="alert"
          className="border-danger/30 bg-danger/10 text-danger m-2 rounded-md border px-2.5 py-2 text-[11px] leading-relaxed"
        >
          {erro}
        </p>
      ) : null}

      {/* ---- o grafo, e o painel lateral por cima dele ---- */}
      <div ref={alvo} className="relative min-h-0 flex-1">
        {total === 0 && !carregando ? (
          <div className="text-muted flex h-full flex-col items-center justify-center gap-1 px-6 text-center">
            <p className="text-[11px] tracking-[0.18em] uppercase">nada aprendido ainda</p>
            <p className="text-muted/70 text-[10px] leading-relaxed">
              Cada conversa que ensina algo vira uma nota, e cada nota vira um ponto aqui.
            </p>
          </div>
        ) : (
          tamanho.largura > 0 && <Grafo largura={tamanho.largura} altura={tamanho.altura} />
        )}

        {aberto ? <PainelDoNo onFechar={fechar} /> : null}
      </div>
    </div>
  )
}

function Medidor({
  rotulo,
  valor,
  detalhe,
}: {
  rotulo: string
  valor: number
  detalhe?: string
}) {
  return (
    <div className="flex shrink-0 flex-col leading-none">
      <span className="text-accent text-sm font-medium tabular-nums">{valor}</span>
      <span className="text-muted/70 text-[9px] tracking-[0.12em] uppercase">
        {rotulo}
        {detalhe ? ` · ${detalhe}` : ''}
      </span>
    </div>
  )
}

/** O que o Jarvis aprendeu sobre um assunto, quando se clica no ponto dele. */
function PainelDoNo({ onFechar }: { onFechar: () => void }) {
  const aberto = useConhecimentoStore((state) => state.aberto)
  if (!aberto) return null

  const { no, corpo } = aberto

  return (
    <aside className="border-border-soft bg-surface/95 absolute inset-y-0 right-0 flex w-64 flex-col border-l backdrop-blur-sm">
      <header className="border-border-soft flex items-start gap-2 border-b px-3 py-2">
        <div className="min-w-0 flex-1">
          <h3 className="text-content truncate text-xs font-medium">{no.rotulo}</h3>
          <p className="text-muted/70 text-[10px]">
            {no.tipo} · {no.tamanho} chars · {no.citacoes} citações · {no.atualizado}
          </p>
        </div>
        <button
          type="button"
          onClick={onFechar}
          aria-label="Fechar detalhes"
          className="text-muted hover:text-danger shrink-0 text-[10px]"
        >
          ✕
        </button>
      </header>

      {/* O peso é o "nível de conhecimento". A barra existe porque o número sozinho não
          diz nada — ao lado do tamanho e das citações, ele passa a dizer. */}
      <div className="border-border-soft border-b px-3 py-2">
        <div className="text-muted/70 mb-1 flex justify-between text-[9px] tracking-[0.12em] uppercase">
          <span>nível de conhecimento</span>
          <span className="tabular-nums">{Math.round(no.peso * 100)}%</span>
        </div>
        <div className="bg-border-soft h-1 overflow-hidden rounded-full">
          <div className="bg-accent h-full rounded-full" style={{ width: `${no.peso * 100}%` }} />
        </div>
      </div>

      <div className="scroll-thin min-h-0 flex-1 overflow-y-auto px-3 py-2">
        {corpo ? (
          <p className="text-muted text-[11px] leading-relaxed whitespace-pre-wrap">{corpo}</p>
        ) : (
          <p className="text-muted/60 text-[11px]">carregando…</p>
        )}
      </div>
    </aside>
  )
}
