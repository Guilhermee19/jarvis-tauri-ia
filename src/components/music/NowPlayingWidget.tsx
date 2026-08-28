'use client'

import { useCallback, useEffect, useRef, type PointerEvent as ReactPointerEvent } from 'react'
import { NextIcon, PauseIcon, PlayIcon, PrevIcon } from '@/components/ui/icons'
import { pressMediaKey, type MediaKey } from '@/lib/tauri'
import { cn } from '@/lib/utils'
import { useJanelaStore, useNowPlayingStore, zDaJanela, type WidgetPosition } from '@/stores'

/**
 * Cartão de "tocando agora", arrastável dentro da janela do Jarvis.
 *
 * Aparece sozinho quando o Jarvis põe uma música para tocar e some no ✕. Os botões
 * mandam tecla de mídia global — quem obedece é o player em foco, então funcionam
 * mesmo depois que você trocou de faixa por fora.
 *
 * ponytail: o arrasto repete a lógica do `FloatingPanel`, ~25 linhas. Duas cópias
 * ainda cabem; na terceira, extrair para um `usePointerDrag`.
 */
export function NowPlayingWidget() {
  const faixa = useNowPlayingStore((state) => state.faixa)
  const tocando = useNowPlayingStore((state) => state.tocando)
  const decorridoMs = useNowPlayingStore((state) => state.decorridoMs)
  const posicaoConfiavel = useNowPlayingStore((state) => state.posicaoConfiavel)
  const posicao = useNowPlayingStore((state) => state.posicao)
  const mover = useNowPlayingStore((state) => state.mover)
  const fechar = useNowPlayingStore((state) => state.fechar)

  const cartaoRef = useRef<HTMLDivElement>(null)
  const pegadaRef = useRef<WidgetPosition | null>(null)

  const abertas = useJanelaStore((state) => state.abertas)
  const abrirJanela = useJanelaStore((state) => state.abrir)
  const fecharJanela = useJanelaStore((state) => state.fechar)

  // Este cartão não é aberto por um botão: ele existe enquanto houver faixa. Mas
  // participa do mesmo empilhamento das outras janelinhas — senão ele fica eternamente
  // por cima (era um `z-40` fixo) e não há como trazer a conversa para a frente dele.
  //
  // A dependência é `faixa !== null` e não `faixa`: trocar de música criaria um objeto
  // novo, e o cartão pularia para a frente sozinho a cada faixa.
  const tocandoAlgo = faixa !== null
  useEffect(() => {
    if (tocandoAlgo) abrirJanela('musica')
    else fecharJanela('musica')
  }, [tocandoAlgo, abrirJanela, fecharJanela])

  const prender = useCallback((x: number, y: number): WidgetPosition => {
    const cartao = cartaoRef.current
    const area = cartao?.offsetParent
    if (!cartao || !(area instanceof HTMLElement)) return { x, y }

    return {
      x: Math.min(Math.max(x, 0), Math.max(0, area.clientWidth - cartao.offsetWidth)),
      y: Math.min(Math.max(y, 0), Math.max(0, area.clientHeight - cartao.offsetHeight)),
    }
  }, [])

  function comecarArrasto(event: ReactPointerEvent<HTMLElement>) {
    // Encostar traz para a frente, mesmo que o alvo seja um botão e não vire arrasto —
    // por isso vem antes da guarda abaixo.
    abrirJanela('musica')

    // Os botões moram dentro do cartão: clicar neles não é arrastar.
    if (event.button !== 0 || (event.target as HTMLElement).closest('button')) return

    const cartao = cartaoRef.current
    if (!cartao) return

    const caixa = cartao.getBoundingClientRect()
    pegadaRef.current = { x: event.clientX - caixa.left, y: event.clientY - caixa.top }
    event.currentTarget.setPointerCapture(event.pointerId)
  }

  function arrastar(event: ReactPointerEvent<HTMLElement>) {
    const pegada = pegadaRef.current
    const cartao = cartaoRef.current
    const area = cartao?.offsetParent
    if (!pegada || !cartao || !(area instanceof HTMLElement)) return

    const caixa = area.getBoundingClientRect()
    mover(prender(event.clientX - caixa.left - pegada.x, event.clientY - caixa.top - pegada.y))
  }

  function soltar(event: ReactPointerEvent<HTMLElement>) {
    pegadaRef.current = null
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
  }

  if (!faixa) return null

  const progresso = faixa.duracaoMs > 0 ? (decorridoMs / faixa.duracaoMs) * 100 : 0

  return (
    <div
      ref={cartaoRef}
      onPointerDown={comecarArrasto}
      onPointerMove={arrastar}
      onPointerUp={soltar}
      onPointerCancel={soltar}
      style={{
        zIndex: zDaJanela(abertas, 'musica'),
        ...(posicao ? { left: posicao.x, top: posicao.y } : {}),
      }}
      className={cn(
        'no-select border-accent/25 bg-surface/20 absolute flex w-[300px] cursor-grab',
        'flex-col gap-2 rounded-xl border p-3 shadow-2xl shadow-black/60 backdrop-blur-xs',
        'active:cursor-grabbing',
        // Enquanto ninguém arrastou, nasce no canto de baixo à direita — longe do
        // núcleo do HUD, que fica no meio.
        !posicao && 'right-3 bottom-3',
      )}
      role="region"
      aria-label="Tocando agora"
    >
      <div className="flex items-start gap-3">
        <Capa url={faixa.capa} />

        <div className="min-w-0 flex-1">
          <p className="text-content truncate text-[13px] leading-tight font-medium">
            {faixa.titulo}
          </p>
          <p className="text-muted truncate text-[11px] leading-tight">{faixa.artista}</p>
        </div>

        <button
          type="button"
          onClick={fechar}
          aria-label="Fechar"
          className="text-muted hover:bg-surface-hover hover:text-content -mt-1 flex h-5 w-5 shrink-0 items-center justify-center self-start rounded text-[10px] transition-colors"
        >
          ✕
        </button>
      </div>

      <div className="-mt-5 flex items-center justify-center gap-2">
        <Transporte tecla="previous" rotulo="Faixa anterior">
          <PrevIcon className="h-3.5 w-3.5" />
        </Transporte>

        <Transporte tecla="play-pause" rotulo={tocando ? 'Pausar' : 'Tocar'} destaque>
          {tocando ? <PauseIcon className="h-4 w-4" /> : <PlayIcon className="h-4 w-4" />}
        </Transporte>

        <Transporte tecla="next" rotulo="Próxima faixa">
          <NextIcon className="h-3.5 w-3.5" />
        </Transporte>
      </div>

      {/* A barra só aparece quando sabemos ONDE a música está — ou seja, quando vimos
          ela começar. Com o app aberto no meio de uma faixa, mostrar a barra em zero
          seria inventar uma posição; aí fica só a duração total. */}
      {faixa.duracaoMs > 0 ? (
        <div className="flex items-center gap-2">
          {posicaoConfiavel ? (
            <>
              <Tempo ms={decorridoMs} />
              <div className="bg-border-soft h-1 flex-1 overflow-hidden rounded-full">
                <div
                  className="bg-accent h-full rounded-full transition-[width] duration-1000 ease-linear"
                  style={{ width: `${Math.min(100, progresso)}%` }}
                />
              </div>
            </>
          ) : (
            <span className="text-muted/60 flex-1 text-[10px]">já estava tocando</span>
          )}
          <Tempo ms={faixa.duracaoMs} />
        </div>
      ) : null}
    </div>
  )
}

function Capa({ url }: { url: string | null }) {
  if (!url) {
    return <div className="border-border-soft bg-base h-14 w-14 shrink-0 rounded-md border" />
  }

  return (
    // `next/image` não tem o que otimizar: a URL é do CDN do Spotify, o app é export
    // estático e não há servidor para redimensionar nada.
    // eslint-disable-next-line @next/next/no-img-element
    <img
      src={url}
      alt=""
      className="border-border-soft h-14 w-14 shrink-0 rounded-md border object-cover"
    />
  )
}

function Tempo({ ms }: { ms: number }) {
  const total = Math.floor(ms / 1000)
  const minutos = Math.floor(total / 60)
  const segundos = String(total % 60).padStart(2, '0')

  // Tabular para o número não dançar a cada segundo e empurrar a barra.
  return (
    <span className="text-muted w-[34px] text-[10px] tabular-nums">
      {minutos}:{segundos}
    </span>
  )
}

function Transporte({
  tecla,
  rotulo,
  destaque,
  children,
}: {
  tecla: MediaKey
  rotulo: string
  destaque?: boolean
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      onClick={() => void pressMediaKey(tecla).catch(() => {})}
      aria-label={rotulo}
      title={rotulo}
      className={cn(
        'flex items-center justify-center rounded-md transition-colors',
        destaque
          ? 'bg-accent-strong hover:bg-accent h-8 w-8 text-white'
          : 'text-muted hover:bg-surface-hover hover:text-content h-7 w-7',
      )}
    >
      {children}
    </button>
  )
}
