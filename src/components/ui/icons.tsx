/**
 * Ícones inline em SVG.
 *
 * Sem biblioteca de ícones: são poucos, e `currentColor` já resolve o tema.
 * Se a contagem passar de ~15, trocar por `lucide-react` de uma vez.
 */

interface IconProps {
  className?: string
}

function Svg({ className, children }: IconProps & { children: React.ReactNode }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.7}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
    >
      {children}
    </svg>
  )
}

export function ChatIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M20.5 12c0 4-3.8 7.2-8.5 7.2a9.9 9.9 0 0 1-2.6-.34L4.5 20.5l1.2-3.5A6.9 6.9 0 0 1 3.5 12c0-4 3.8-7.2 8.5-7.2s8.5 3.2 8.5 7.2Z" />
    </Svg>
  )
}

export function SettingsIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M4 6.5h8M17 6.5h3M4 12h3M12 12h8M4 17.5h8M17 17.5h3" />
      <circle cx="14.5" cy="6.5" r="2.1" />
      <circle cx="9.5" cy="12" r="2.1" />
      <circle cx="14.5" cy="17.5" r="2.1" />
    </Svg>
  )
}

export function MicIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <rect x="9" y="3" width="6" height="11" rx="3" />
      <path d="M5.5 11.5a6.5 6.5 0 0 0 13 0M12 18v3" />
    </Svg>
  )
}

export function CameraIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M3.5 8.5h3l1.4-2h7.2l1.4 2h3v10h-16Z" />
      <circle cx="11.5" cy="13" r="3.2" />
    </Svg>
  )
}

export function ScreenIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <rect x="3" y="4.5" width="18" height="12" rx="2" />
      <path d="M9 20h6M12 16.5V20" />
    </Svg>
  )
}

/** Sinal de vida: a aba de diagnóstico é sobre sensores respondendo ou não. */
export function PulseIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M3 12h3.5l2-5.5 3.5 11 2.5-7 1.8 3.5H21" />
    </Svg>
  )
}

export function PowerIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M12 3.5v7.5" />
      <path d="M6.9 6.6a7 7 0 1 0 10.2 0" />
    </Svg>
  )
}
