'use client'

import {
  CameraIcon,
  ChatIcon,
  MicIcon,
  PowerIcon,
  PulseIcon,
  SettingsIcon,
} from '@/components/ui/icons'
import { quitApp } from '@/lib/tauri'
import { cn } from '@/lib/utils'
import { useChatStore, useSensorStore, useSheetStore, type SheetId } from '@/stores'

interface NavItem {
  id: SheetId
  label: string
  Icon: (props: { className?: string }) => React.ReactElement
}

/**
 * Barra de ícones. Dois grupos com semânticas diferentes:
 *
 * - Sensores (webcam, microfone): interruptores de algo que continua ligado depois
 *   do clique, independente da tela aberta.
 * - Gavetas (conversa, diagnóstico, configurações): barra de tarefas — clicar abre,
 *   clicar de novo fecha, e é assim que se volta para o núcleo.
 */
const ITEMS: NavItem[] = [
  { id: 'chat', label: 'Conversa', Icon: ChatIcon },
  { id: 'diagnostics', label: 'Diagnóstico', Icon: PulseIcon },
  { id: 'settings', label: 'Configurações', Icon: SettingsIcon },
]

export function BottomNav() {
  const activeSheet = useSheetStore((state) => state.activeSheet)
  const toggle = useSheetStore((state) => state.toggle)
  const hasMessages = useChatStore((state) => state.messages.length > 0)

  return (
    <nav className="no-select border-border-soft bg-surface/80 flex shrink-0 items-center justify-center gap-1 border-t px-3 py-2 backdrop-blur-sm">
      <WebcamButton />
      <MicButton />

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

function WebcamButton() {
  const isOn = useSensorStore((state) => state.isWebcamOn)
  const isBusy = useSensorStore((state) => state.isWebcamBusy)
  const toggleWebcam = useSensorStore((state) => state.toggleWebcam)

  return (
    <NavButton
      label={isOn ? 'Desligar a webcam' : 'Ligar a webcam'}
      isActive={isOn}
      isBusy={isBusy}
      onClick={() => void toggleWebcam()}
      icon={<CameraIcon className="h-4.5 w-4.5" />}
    />
  )
}

function MicButton() {
  const isOn = useSensorStore((state) => state.isMicOn)
  const isBusy = useSensorStore((state) => state.isMicBusy)
  const level = useSensorStore((state) => state.micLevel)
  const toggleMic = useSensorStore((state) => state.toggleMic)

  return (
    <NavButton
      label={isOn ? 'Desligar o microfone' : 'Ligar o microfone'}
      isActive={isOn}
      isBusy={isBusy}
      onClick={() => void toggleMic()}
      icon={<MicIcon className="h-4.5 w-4.5" />}
      // Anel que respira com a voz: o botão precisa mostrar que está CAPTANDO, não
      // só que está ligado. A raiz quadrada tira a fala do fundo da escala linear.
      ring={isOn ? Math.min(1, Math.sqrt(level)) : null}
    />
  )
}

interface NavButtonProps {
  label: string
  isActive: boolean
  hasDot?: boolean
  isDanger?: boolean
  isBusy?: boolean
  ring?: number | null
  onClick: () => void
  icon: React.ReactNode
}

function NavButton({
  label,
  isActive,
  hasDot,
  isDanger,
  isBusy,
  ring,
  onClick,
  icon,
}: NavButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={isBusy}
      title={label}
      aria-label={label}
      aria-pressed={isActive}
      className={cn(
        'relative flex h-8 w-9 items-center justify-center rounded transition-colors',
        'disabled:cursor-not-allowed disabled:opacity-60',
        isActive && 'hud-glow bg-accent/10 text-accent',
        !isActive && (isDanger ? 'text-muted hover:text-danger' : 'text-muted hover:text-content'),
      )}
    >
      {ring != null ? (
        <span
          aria-hidden
          className="border-accent pointer-events-none absolute inset-0.5 rounded border transition-opacity duration-75"
          style={{ opacity: 0.15 + ring * 0.85 }}
        />
      ) : null}

      {icon}

      {hasDot ? (
        <span className="bg-accent absolute top-1.5 right-1.5 h-1 w-1 rounded-full" />
      ) : null}
    </button>
  )
}
