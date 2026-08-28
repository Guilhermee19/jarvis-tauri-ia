'use client'

import { CameraIcon, ChatIcon, HouseIcon, MicIcon, SettingsIcon } from '@/components/ui/icons'
import { cn } from '@/lib/utils'
import { useChatStore, useJanelaStore, useSensorStore, type JanelaId } from '@/stores'

interface NavItem<Id> {
  id: Id
  label: string
  Icon: (props: { className?: string }) => React.ReactElement
}

/**
 * Barra de ícones. Dois grupos com semânticas diferentes:
 *
 * - Sensores (webcam, microfone): interruptores de algo que continua ligado depois
 *   do clique, independente da tela aberta.
 * - Gavetas (configurações): barra de tarefas — clicar abre,
 *   clicar de novo fecha, e é assim que se volta para o núcleo.
 */
/**
 * Janelinhas: convivem, então o botão aceso quer dizer "está aberta" — várias podem
 * estar. Clicar segue a semântica de barra de tarefas do `janelaStore`.
 */
const JANELAS: NavItem<Exclude<JanelaId, 'musica'>>[] = [
  { id: 'chat', label: 'Conversa', Icon: ChatIcon },
  { id: 'casa', label: 'Casa', Icon: HouseIcon },
]

/** Gavetas: uma por vez, então o botão aceso quer dizer "é esta que está aberta". */
const GAVETAS: NavItem<'settings'>[] = [
  { id: 'settings', label: 'Configurações', Icon: SettingsIcon },
]

export function BottomNav() {
  const abertas = useJanelaStore((state) => state.abertas)
  const alternar = useJanelaStore((state) => state.alternar)
  const gaveta = useJanelaStore((state) => state.gaveta)
  const alternarGaveta = useJanelaStore((state) => state.alternarGaveta)
  const hasMessages = useChatStore((state) => state.messages.length > 0)

  return (
    <nav className="no-select flex shrink-0 items-center justify-center gap-1 px-3 py-2">
      <WebcamButton />
      <MicButton />

      <span className="bg-border-soft mx-1 h-4 w-px" />

      {JANELAS.map(({ id, label, Icon }) => (
        <NavButton
          key={id}
          label={label}
          isActive={abertas.includes(id)}
          hasDot={id === 'chat' && hasMessages}
          onClick={() => alternar(id)}
          icon={<Icon className="h-4.5 w-4.5" />}
        />
      ))}

      <span className="bg-border-soft mx-1 h-4 w-px" />

      {GAVETAS.map(({ id, label, Icon }) => (
        <NavButton
          key={id}
          label={label}
          isActive={gaveta === id}
          onClick={() => alternarGaveta(id)}
          icon={<Icon className="h-4.5 w-4.5" />}
        />
      ))}
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
  isBusy?: boolean
  ring?: number | null
  onClick: () => void
  icon: React.ReactNode
}

function NavButton({ label, isActive, hasDot, isBusy, ring, onClick, icon }: NavButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={isBusy}
      title={label}
      aria-label={label}
      aria-pressed={isActive}
      className={cn(
        'border-border-soft bg-surface/80 relative flex h-9 w-9 cursor-pointer items-center justify-center rounded-full border backdrop-blur-sm transition-colors',
        'disabled:cursor-not-allowed disabled:opacity-60',
        isActive && 'hud-glow bg-accent/10 text-accent',
        !isActive && 'text-muted hover:text-content',
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
