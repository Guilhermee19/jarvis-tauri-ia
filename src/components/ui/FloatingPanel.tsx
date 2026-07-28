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

interface FloatingPanelProps {
  open: boolean
  title: string
  /** Lido por leitor de tela; não aparece na UI. */
  description: string
  /** Botões extras no cabeçalho, à esquerda do fechar. */
  actions?: React.ReactNode
  /** `null` enquanto ninguém arrastou: a janelinha nasce centralizada. */
  position: PanelPosition | null
  onPositionChange: (position: PanelPosition) => void
  onClose: () => void
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
  position,
  onPositionChange,
  onClose,
  children,
}: FloatingPanelProps) {
  const panelRef = useRef<HTMLDivElement>(null)
  /** Distância do ponteiro até o canto do painel, congelada no início do arrasto. */
  const grabRef = useRef<PanelPosition | null>(null)
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

  // A janela do app é redimensionável: encolher não pode empurrar a janelinha
  // para fora do alcance do mouse.
  useEffect(() => {
    if (!open || !position) return

    function onResize() {
      if (position) onPositionChange(clampIntoBounds(position.x, position.y))
    }

    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  }, [open, position, onPositionChange, clampIntoBounds])

  function startDrag(event: ReactPointerEvent<HTMLElement>) {
    // Fechar e limpar histórico moram no cabeçalho: clicar neles não é arrastar.
    if (event.button !== 0 || (event.target as HTMLElement).closest('button')) return

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

  if (!open) return null

  return (
    <div
      ref={panelRef}
      role="dialog"
      aria-modal="false"
      aria-label={title}
      aria-describedby={descriptionId}
      tabIndex={-1}
      style={position ? { left: position.x, top: position.y } : undefined}
      className={cn(
        'floating-panel border-accent/25 bg-surface/95 absolute z-30 flex flex-col',
        'h-[min(420px,calc(100%-1.5rem))] w-[min(340px,calc(100%-1.5rem))]',
        'rounded-lg border shadow-2xl shadow-black/60 backdrop-blur-md focus:outline-none',
        // Enquanto ninguém arrastou não há pixel para usar: o centro vem do CSS, e o
        // primeiro arrasto converte a posição real em coordenadas.
        !position && 'top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
      )}
    >
      {/* Sem `data-tauri-drag-region` aqui — ao contrário da `TitleBar`, este
          cabeçalho move a janelinha dentro do app, não a janela do sistema. */}
      <header
        onPointerDown={startDrag}
        onPointerMove={drag}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        className="border-border-soft no-select flex shrink-0 cursor-grab items-center gap-2 rounded-t-lg border-b px-3 py-2 active:cursor-grabbing"
      >
        <span aria-hidden className="text-muted/70 shrink-0 text-[11px] leading-none">
          ⠿
        </span>
        <h2 className="text-muted flex-1 truncate text-[10px] tracking-[0.2em] uppercase">
          {title}
        </h2>

        {actions}

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
    </div>
  )
}
