'use client'

import type { ButtonHTMLAttributes } from 'react'
import { cn } from '@/lib/utils'

type ButtonVariant = 'primary' | 'ghost' | 'subtle'

const VARIANTS: Record<ButtonVariant, string> = {
  primary: 'bg-accent-strong text-white hover:bg-accent disabled:hover:bg-accent-strong',
  subtle: 'bg-surface-hover text-content hover:bg-border-soft',
  ghost: 'bg-transparent text-muted hover:bg-surface-hover hover:text-content',
}

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant
}

export function Button({ variant = 'primary', className, type = 'button', ...props }: ButtonProps) {
  return (
    <button
      type={type}
      className={cn(
        'inline-flex items-center justify-center gap-2 rounded-lg px-3 py-2 text-sm font-medium',
        // `focus-visible` e não `focus`: o anel só aparece para quem chegou pelo
        // TECLADO. Com `focus:ring-2`, clicar deixava a marca acesa até o próximo
        // clique fora — ruído puro, porque quem usa mouse já sabe onde clicou.
        // Tirar o anel de vez seria pior: sem ele, navegar por Tab fica às cegas.
        'focus-visible:ring-accent/50 transition-colors focus-visible:ring-2 focus-visible:outline-none',
        'disabled:cursor-not-allowed disabled:opacity-50',
        VARIANTS[variant],
        className,
      )}
      {...props}
    />
  )
}
