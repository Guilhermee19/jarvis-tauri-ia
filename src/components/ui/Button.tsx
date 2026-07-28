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
        'focus:ring-accent/50 transition-colors focus:ring-2 focus:outline-none',
        'disabled:cursor-not-allowed disabled:opacity-50',
        VARIANTS[variant],
        className,
      )}
      {...props}
    />
  )
}
