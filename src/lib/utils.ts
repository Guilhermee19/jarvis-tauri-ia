type ClassValue = string | false | null | undefined

/** Junta classes condicionais sem puxar `clsx` — o projeto não precisa de mais que isso ainda. */
export function cn(...classes: ClassValue[]): string {
  return classes.filter(Boolean).join(' ')
}

export function formatTime(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString('pt-BR', {
    hour: '2-digit',
    minute: '2-digit',
  })
}
