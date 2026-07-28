'use client'

import { useId, type InputHTMLAttributes } from 'react'
import { cn } from '@/lib/utils'

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label: string
  hint?: string
}

export function Input({ label, hint, className, id, ...props }: InputProps) {
  const generatedId = useId()
  const inputId = id ?? generatedId

  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={inputId} className="text-muted text-xs font-medium">
        {label}
      </label>
      <input
        id={inputId}
        className={cn(
          'border-border-soft bg-base text-content w-full rounded-lg border px-3 py-2 text-sm',
          'placeholder:text-muted/60 focus:border-accent focus:outline-none',
          className,
        )}
        {...props}
      />
      {hint ? <p className="text-muted text-[11px] leading-snug">{hint}</p> : null}
    </div>
  )
}
