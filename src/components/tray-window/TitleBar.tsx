'use client'

import { hideWindow, minimizeWindow } from '@/lib/tauri'
import { useSettingsStore } from '@/stores'

/**
 * Barra de título própria — a janela roda com `decorations: false` para ter cara de
 * widget. O `data-tauri-drag-region` é o que permite arrastar a janela pela barra.
 *
 * Só controles de janela aqui: navegação e configurações moram na `BottomNav`.
 */
export function TitleBar() {
  const assistantName = useSettingsStore((state) => state.settings.assistantName)

  return (
    <header
      data-tauri-drag-region
      className="no-select border-border-soft bg-surface/60 flex shrink-0 items-center justify-between border-b px-3 py-1.5 backdrop-blur-sm"
    >
      <div data-tauri-drag-region className="flex items-center gap-2">
        <span className="hud-glow bg-accent text-accent h-1.5 w-1.5 rounded-full" />
        <span
          data-tauri-drag-region
          className="text-content/90 text-[11px] tracking-[0.22em] uppercase"
        >
          {assistantName}
        </span>
      </div>

      <div className="flex items-center gap-1">
        <TitleBarButton label="Minimizar" onClick={() => void minimizeWindow()}>
          —
        </TitleBarButton>
        {/* O X esconde para a bandeja em vez de encerrar (mesmo comportamento do
            `CloseRequested` interceptado em `src-tauri/src/window.rs`). */}
        <TitleBarButton label="Esconder na bandeja" onClick={() => void hideWindow()}>
          ✕
        </TitleBarButton>
      </div>
    </header>
  )
}

function TitleBarButton({
  label,
  onClick,
  children,
}: {
  label: string
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      onClick={onClick}
      className="text-muted hover:bg-surface-hover hover:text-content flex h-6 w-6 items-center justify-center rounded text-xs transition-colors"
    >
      {children}
    </button>
  )
}
