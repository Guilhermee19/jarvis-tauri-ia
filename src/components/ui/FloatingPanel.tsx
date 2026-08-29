'use client'

import {
  useCallback,
  useEffect,
  useId,
  useRef,
  type PointerEvent as ReactPointerEvent,
} from 'react'
import { cn } from '@/lib/utils'

export interface PanelPosition {
  x: number
  y: number
}

export interface PanelSize {
  width: number
  height: number
}

/** Abaixo disto o cabeçalho e o campo de texto não cabem mais. */
const MINIMO: PanelSize = { width: 280, height: 220 }

/** Passo do redimensionamento pelo teclado. */
const PASSO = 24

interface FloatingPanelProps {
  open: boolean
  title: string
  /** Lido por leitor de tela; não aparece na UI. */
  description: string
  /** Botões extras no cabeçalho, à esquerda do fechar. */
  actions?: React.ReactNode
  /**
   * Fixada reabre sozinha ao subir o app, onde e do tamanho que ficou.
   *
   * `undefined` esconde o botão: nem toda janelinha tem por que oferecer isso, e um
   * botão que não faz nada é pior que a ausência dele.
   */
  fixada?: boolean
  onFixadaChange?: (fixada: boolean) => void
  /** `null` enquanto ninguém arrastou: a janelinha nasce centralizada. */
  position: PanelPosition | null
  onPositionChange: (position: PanelPosition) => void
  /** `null` enquanto ninguém redimensionou: o tamanho vem das classes. */
  size: PanelSize | null
  onSizeChange: (size: PanelSize) => void
  /**
   * Avisa quando o arrasto da alça começa e termina.
   *
   * Existe por causa de UMA janelinha: a do navegador, cujo conteúdo é um webview nativo
   * empilhado acima do HTML. Enquanto a alça é arrastada o ponteiro entra na área dele, e
   * uma camada nativa engole o evento do mouse antes que o HTML o veja — o arrasto morre no
   * meio. Escondendo o webview durante o arrasto, o problema não existe.
   *
   * Opcional porque as outras janelinhas são HTML puro e não precisam saber disso.
   */
  onResizingChange?: (redimensionando: boolean) => void
  /** Ocupa a área inteira. Posição e tamanho ficam GUARDADOS para o restaurar. */
  maximized: boolean
  onMaximizedChange: (maximized: boolean) => void
  onClose: () => void
  /** Empilhamento entre janelinhas abertas. Quem manda é o `janelaStore`. */
  zIndex: number
  /**
   * Chamado quando alguém encosta em qualquer parte da janelinha, para ela vir à frente.
   *
   * `onPointerDown` e não `onClick`: a janela precisa subir ANTES do arrasto começar,
   * senão o primeiro movimento acontece com ela ainda atrás da outra.
   */
  onFocus: () => void
  children: React.ReactNode
}

/**
 * Janelinha flutuante que o usuário arrasta pelo cabeçalho, presa à área de
 * conteúdo da janela do app.
 *
 * Não é um Dialog do Radix de propósito: Dialog fecha ao clicar fora e assume que
 * a coisa aberta é a única que importa. Aqui é o contrário — a janelinha convive
 * com o HUD e com a barra de ícones, e sair dela para clicar em outro lugar é uso
 * normal, não intenção de fechar. O que o Dialog daria de graça (Esc, foco, aria)
 * são as poucas linhas abaixo.
 */
export function FloatingPanel({
  open,
  title,
  description,
  actions,
  fixada,
  onFixadaChange,
  position,
  onPositionChange,
  size,
  onSizeChange,
  onResizingChange,
  maximized,
  onMaximizedChange,
  onClose,
  zIndex,
  onFocus,
  children,
}: FloatingPanelProps) {
  const panelRef = useRef<HTMLDivElement>(null)
  /** Distância do ponteiro até o canto do painel, congelada no início do arrasto. */
  const grabRef = useRef<PanelPosition | null>(null)
  /** Ponteiro e tamanho no instante em que o redimensionamento começou. */
  const resizeRef = useRef<(PanelPosition & PanelSize) | null>(null)
  const descriptionId = useId()

  /** Mantém a janelinha inteira dentro da área de conteúdo — arrastá-la para fora
   *  deixaria o cabeçalho inalcançável, sem como trazê-la de volta. */
  const clampIntoBounds = useCallback((x: number, y: number): PanelPosition => {
    const panel = panelRef.current
    const bounds = panel?.offsetParent
    if (!panel || !(bounds instanceof HTMLElement)) return { x, y }

    const maxX = Math.max(0, bounds.clientWidth - panel.offsetWidth)
    const maxY = Math.max(0, bounds.clientHeight - panel.offsetHeight)
    return {
      x: Math.min(Math.max(x, 0), maxX),
      y: Math.min(Math.max(y, 0), maxY),
    }
  }, [])

  /** Nem menor que o mínimo utilizável, nem maior que o espaço à direita e abaixo. */
  const clampSize = useCallback((width: number, height: number): PanelSize => {
    const panel = panelRef.current
    const bounds = panel?.offsetParent
    if (!panel || !(bounds instanceof HTMLElement)) return { width, height }

    const maxWidth = Math.max(MINIMO.width, bounds.clientWidth - panel.offsetLeft)
    const maxHeight = Math.max(MINIMO.height, bounds.clientHeight - panel.offsetTop)

    return {
      width: Math.min(Math.max(width, MINIMO.width), maxWidth),
      height: Math.min(Math.max(height, MINIMO.height), maxHeight),
    }
  }, [])

  useEffect(() => {
    if (!open) return

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') onClose()
    }

    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [open, onClose])

  // O Dialog do Radix focava o primeiro campo ao abrir; sem ele, isso é por nossa conta.
  useEffect(() => {
    if (!open) return
    const panel = panelRef.current
    const field = panel?.querySelector<HTMLElement>('textarea, input')
    ;(field ?? panel)?.focus()
  }, [open])

  // A janela do app é redimensionável: encolher não pode empurrar a janelinha para
  // fora do alcance do mouse, nem deixá-la maior que a área que sobrou.
  useEffect(() => {
    if (!open || (!position && !size)) return

    function onResize() {
      if (position) onPositionChange(clampIntoBounds(position.x, position.y))
      if (size) onSizeChange(clampSize(size.width, size.height))
    }

    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  }, [open, position, onPositionChange, clampIntoBounds, size, onSizeChange, clampSize])

  function startDrag(event: ReactPointerEvent<HTMLElement>) {
    // Fechar e limpar histórico moram no cabeçalho: clicar neles não é arrastar.
    // Maximizada não se arrasta — ela ocupa tudo, não há para onde ir.
    if (maximized || event.button !== 0 || (event.target as HTMLElement).closest('button')) return

    const panel = panelRef.current
    if (!panel) return

    const rect = panel.getBoundingClientRect()
    grabRef.current = { x: event.clientX - rect.left, y: event.clientY - rect.top }
    event.currentTarget.setPointerCapture(event.pointerId)
  }

  function drag(event: ReactPointerEvent<HTMLElement>) {
    const grab = grabRef.current
    const panel = panelRef.current
    const bounds = panel?.offsetParent
    if (!grab || !panel || !(bounds instanceof HTMLElement)) return

    const boundsRect = bounds.getBoundingClientRect()
    onPositionChange(
      clampIntoBounds(
        event.clientX - boundsRect.left - grab.x,
        event.clientY - boundsRect.top - grab.y,
      ),
    )
  }

  function endDrag(event: ReactPointerEvent<HTMLElement>) {
    grabRef.current = null
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
  }

  function startResize(event: ReactPointerEvent<HTMLElement>) {
    if (event.button !== 0) return

    const panel = panelRef.current
    const bounds = panel?.offsetParent
    if (!panel || !(bounds instanceof HTMLElement)) return

    // Sem posição explícita a janelinha está centralizada por `translate`, e crescer
    // pelo canto empurraria os DOIS lados. Fixar a posição atual antes de começar faz
    // o canto de baixo à direita se comportar como no Windows: só ele se move.
    if (!position) {
      const rect = panel.getBoundingClientRect()
      const boundsRect = bounds.getBoundingClientRect()
      onPositionChange({ x: rect.left - boundsRect.left, y: rect.top - boundsRect.top })
    }

    resizeRef.current = {
      x: event.clientX,
      y: event.clientY,
      width: panel.offsetWidth,
      height: panel.offsetHeight,
    }
    event.currentTarget.setPointerCapture(event.pointerId)
    event.stopPropagation()
    onResizingChange?.(true)
  }

  function resize(event: ReactPointerEvent<HTMLElement>) {
    const inicio = resizeRef.current
    if (!inicio) return

    onSizeChange(
      clampSize(
        inicio.width + (event.clientX - inicio.x),
        inicio.height + (event.clientY - inicio.y),
      ),
    )
  }

  function endResize(event: ReactPointerEvent<HTMLElement>) {
    resizeRef.current = null
    onResizingChange?.(false)
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
  }

  /** Arrastar um canto não funciona no teclado, e sem isto a alça seria decorativa
   *  para quem não usa mouse. */
  function resizeByKeyboard(event: React.KeyboardEvent<HTMLElement>) {
    const passos: Record<string, PanelSize> = {
      ArrowRight: { width: PASSO, height: 0 },
      ArrowLeft: { width: -PASSO, height: 0 },
      ArrowDown: { width: 0, height: PASSO },
      ArrowUp: { width: 0, height: -PASSO },
    }

    const passo = passos[event.key]
    const panel = panelRef.current
    if (!passo || !panel) return

    event.preventDefault()
    onSizeChange(clampSize(panel.offsetWidth + passo.width, panel.offsetHeight + passo.height))
  }

  if (!open) return null

  return (
    <div
      ref={panelRef}
      role="dialog"
      aria-modal="false"
      aria-label={title}
      aria-describedby={descriptionId}
      tabIndex={-1}
      // Encostar em qualquer lugar traz para a frente. No cabeçalho isso acontece antes
      // do `startDrag` porque o evento sobe do filho para cá — a janela já está no topo
      // quando o primeiro pixel de arrasto é processado.
      onPointerDown={onFocus}
      style={
        maximized
          ? { left: 0, top: 0, width: '100%', height: '100%', zIndex }
          : {
              zIndex,
              ...(position ? { left: position.x, top: position.y } : {}),
              ...(size ? { width: size.width, height: size.height } : {}),
            }
      }
      className={cn(
        'floating-panel border-accent/25 bg-surface/20 absolute flex flex-col',
        'border shadow-2xl shadow-black/60 backdrop-blur-xs focus:outline-none',
        // Maximizada encosta nas bordas: canto arredondado ali vira falha de pintura.
        maximized ? 'rounded-none' : 'rounded-lg',
        // Só valem enquanto ninguém redimensionou — depois disso o `style` manda.
        !maximized && !size && 'h-[min(420px,calc(100%-1.5rem))] w-[min(340px,calc(100%-1.5rem))]',
        // Enquanto ninguém arrastou não há pixel para usar: o centro vem do CSS, e o
        // primeiro arrasto converte a posição real em coordenadas.
        !maximized && !position && 'top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
      )}
    >
      {/* Sem `data-tauri-drag-region` aqui — ao contrário da `TitleBar`, este
          cabeçalho move a janelinha dentro do app, não a janela do sistema. */}
      <header
        onPointerDown={startDrag}
        onPointerMove={drag}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        // Duplo clique alterna maximizar, como na barra de título do Windows.
        onDoubleClick={(event) => {
          if (!(event.target as HTMLElement).closest('button')) onMaximizedChange(!maximized)
        }}
        className={cn(
          'border-border-soft no-select flex shrink-0 items-center gap-2 border-b px-3 py-2',
          maximized ? 'cursor-default' : 'cursor-grab rounded-t-lg active:cursor-grabbing',
        )}
      >
        <span aria-hidden className="text-muted/70 shrink-0 text-[11px] leading-none">
          ⠿
        </span>
        <h2 className="text-muted flex-1 truncate text-[10px] tracking-[0.2em] uppercase">
          {title}
        </h2>

        {actions}

        {/* Antes do maximizar porque é uma decisão de OUTRA natureza: maximizar e fechar
            valem para agora, fixar vale para a próxima vez que o app abrir. */}
        {onFixadaChange ? (
          <button
            type="button"
            onClick={() => onFixadaChange(!fixada)}
            aria-pressed={fixada}
            aria-label={fixada ? 'Não abrir sozinha' : 'Abrir sozinha ao iniciar'}
            title={
              fixada
                ? 'Fixada: reabre sozinha, onde e do tamanho que ficar'
                : 'Fixar: reabrir sozinha ao iniciar o Jarvis'
            }
            className={cn(
              'flex h-5 w-5 shrink-0 items-center justify-center rounded text-[10px] transition-colors',
              fixada
                ? 'text-accent bg-accent/15'
                : 'text-muted hover:bg-surface-hover hover:text-content',
            )}
          >
            {/* Cheio quando fixada, vazado quando não: a forma carrega o estado, e não
                só a cor — que some para quem não a distingue. */}
            {fixada ? '★' : '☆'}
          </button>
        ) : null}

        <button
          type="button"
          onClick={() => onMaximizedChange(!maximized)}
          aria-label={maximized ? 'Restaurar o tamanho' : 'Maximizar'}
          title={maximized ? 'Restaurar' : 'Maximizar'}
          className="text-muted hover:bg-surface-hover hover:text-content flex h-5 w-5 shrink-0 items-center justify-center rounded text-[10px] transition-colors"
        >
          {maximized ? '❐' : '▢'}
        </button>

        <button
          type="button"
          onClick={onClose}
          aria-label="Fechar"
          className="text-muted hover:bg-surface-hover hover:text-content flex h-5 w-5 shrink-0 items-center justify-center rounded text-[10px] transition-colors"
        >
          ✕
        </button>
      </header>

      <p id={descriptionId} className="sr-only">
        {description}
      </p>

      <div className="flex min-h-0 flex-1 flex-col">{children}</div>

      {/* Alça no canto de baixo à direita, como nas janelas do Windows. É um `button`
          de verdade para o teclado alcançar — as setas redimensionam.
          Some quando maximizada: não há o que redimensionar ocupando tudo. */}
      {maximized ? null : (
        <button
          type="button"
          onPointerDown={startResize}
          onPointerMove={resize}
          onPointerUp={endResize}
          onPointerCancel={endResize}
          onKeyDown={resizeByKeyboard}
          aria-label="Redimensionar a janela (use as setas)"
          className="text-muted/50 hover:text-accent focus-visible:text-accent absolute right-0 bottom-0 flex h-4 w-4 cursor-nwse-resize items-center justify-center rounded-br-lg text-[9px] leading-none transition-colors focus:outline-none"
        >
          ◢
        </button>
      )}
    </div>
  )
}
