'use client'

import * as DialogPrimitive from '@radix-ui/react-dialog'
import { cn } from '@/lib/utils'

/**
 * Gaveta no estilo do Sheet do shadcn, sobre o mesmo primitivo (Radix Dialog):
 * Esc fecha, clique fora fecha, foco e aria corretos.
 *
 * Duas diferenças conscientes em relação ao shadcn:
 *
 * 1. `modal={false}` — o modal do Radix desliga o ponteiro fora do conteúdo, o que
 *    deixaria a barra de ícones inclicável. Como a barra é justamente o jeito de
 *    trocar de gaveta, ela precisa continuar viva.
 * 2. Sem `Portal` — overlay e conteúdo ficam dentro da área de conteúdo da janela,
 *    então a barra de título (que arrasta a janela) e a de ícones não são cobertas.
 */
export const Sheet = DialogPrimitive.Root
export const SheetClose = DialogPrimitive.Close

interface SheetContentProps {
  title: string
  /** Lido por leitor de tela; não aparece na UI. */
  description: string
  /** Botões extras no cabeçalho, à esquerda do fechar. */
  actions?: React.ReactNode
  children: React.ReactNode
}

export function SheetContent({ title, description, actions, children }: SheetContentProps) {
  return (
    <>
      <DialogPrimitive.Overlay className="sheet-overlay absolute inset-0 bg-black/55 backdrop-blur-[1px]" />

      <DialogPrimitive.Content
        className={cn(
          'sheet-panel border-accent/25 bg-surface/95 absolute inset-y-0 right-0 z-30 flex',
          'w-full max-w-[380px] flex-col border-l shadow-2xl shadow-black/60 backdrop-blur-md',
          'focus:outline-none',
        )}
      >
        <header className="border-border-soft no-select flex shrink-0 items-center gap-2 border-b px-3 py-2">
          <span className="bg-accent h-1.5 w-1.5 shrink-0 rounded-full" />
          <DialogPrimitive.Title className="text-muted flex-1 truncate text-[10px] tracking-[0.2em] uppercase">
            {title}
          </DialogPrimitive.Title>

          {actions}

          <DialogPrimitive.Close
            aria-label="Fechar"
            className="text-muted hover:bg-surface-hover hover:text-content flex h-5 w-5 shrink-0 items-center justify-center rounded text-[10px] transition-colors"
          >
            ✕
          </DialogPrimitive.Close>
        </header>

        <DialogPrimitive.Description className="sr-only">{description}</DialogPrimitive.Description>

        <div className="flex min-h-0 flex-1 flex-col">{children}</div>
      </DialogPrimitive.Content>
    </>
  )
}
