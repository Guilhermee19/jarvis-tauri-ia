'use client'

export function TypingIndicator({ assistantName }: { assistantName: string }) {
  return (
    <div className="flex items-center gap-2 px-1">
      <div className="border-border-soft bg-surface flex gap-1 rounded-2xl rounded-bl-md border px-3.5 py-2.5">
        {[0, 1, 2].map((index) => (
          <span
            key={index}
            className="animate-bounce-dot bg-muted h-1.5 w-1.5 rounded-full"
            style={{ animationDelay: `${index * 0.15}s` }}
          />
        ))}
      </div>
      <span className="text-muted text-[11px]">{assistantName} está pensando…</span>
    </div>
  )
}
