'use client'

import { ChatIcon, HomeIcon, PowerIcon, SettingsIcon } from '@/components/ui/icons'
import { quitApp } from '@/lib/tauri'
import { cn } from '@/lib/utils'
import { useChatStore, useSheetStore, type SheetId } from '@/stores'

interface NavItem {
  id: SheetId
  label: string
  Icon: (props: { className?: string }) => React.ReactElement
}

/**
 * Barra de ícones — funciona como barra de tarefas das gavetas, não como
 * navegação: clicar abre a gaveta, clicar de novo fecha.
 *
 * Features novas (voz, automação) entram nesta lista com o id da gaveta delas.
 */
const ITEMS: NavItem[] = [
  { id: 'chat', label: 'Conversa', Icon: ChatIcon },
  { id: 'settings', label: 'Configurações', Icon: SettingsIcon },
]

export function BottomNav() {
  const activeSheet = useSheetStore((state) => state.activeSheet)
  const toggle = useSheetStore((state) => state.toggle)
  const close = useSheetStore((state) => state.close)
  const hasMessages = useChatStore((state) => state.messages.length > 0)

  return (
    <nav className="no-select border-border-soft bg-surface/80 flex shrink-0 items-center justify-center gap-1 border-t px-3 py-2 backdrop-blur-sm">
      {/* Equivalente ao "mostrar área de trabalho": fecha a gaveta e revela o núcleo. */}
      <NavButton
        label="Mostrar o núcleo"
        isActive={activeSheet === null}
        onClick={close}
        icon={<HomeIcon className="h-4.5 w-4.5" />}
      />

      <span className="bg-border-soft mx-1 h-4 w-px" />

      {ITEMS.map(({ id, label, Icon }) => (
        <NavButton
          key={id}
          label={label}
          isActive={activeSheet === id}
          hasDot={id === 'chat' && hasMessages}
          onClick={() => toggle(id)}
          icon={<Icon className="h-4.5 w-4.5" />}
        />
      ))}

      <span className="bg-border-soft mx-1 h-4 w-px" />

      {/* Encerrar de verdade: o X da barra de título só esconde na bandeja. */}
      <NavButton
        label="Sair do Jarvis"
        isActive={false}
        isDanger
        onClick={() => void quitApp()}
        icon={<PowerIcon className="h-4.5 w-4.5" />}
      />
    </nav>
  )
}

interface NavButtonProps {
  label: string
  isActive: boolean
  hasDot?: boolean
  isDanger?: boolean
  onClick: () => void
  icon: React.ReactNode
}

function NavButton({ label, isActive, hasDot, isDanger, onClick, icon }: NavButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={label}
      aria-label={label}
      aria-pressed={isActive}
      className={cn(
        'relative flex h-8 w-9 items-center justify-center rounded transition-colors',
        isActive && 'hud-glow bg-accent/10 text-accent',
        !isActive && (isDanger ? 'text-muted hover:text-danger' : 'text-muted hover:text-content'),
      )}
    >
      {icon}

      {hasDot ? (
        <span className="bg-accent absolute top-1.5 right-1.5 h-1 w-1 rounded-full" />
      ) : null}
    </button>
  )
}
