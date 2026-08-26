'use client'

import { cn } from '@/lib/utils'

interface JarvisCoreProps {
  label: string
  /** Liga as camadas que giram — desligado dá a leitura de "parado/ocioso". */
  isActive?: boolean
  className?: string
}

/**
 * O núcleo do HUD: anéis concêntricos girando em velocidades diferentes com o
 * wordmark no centro.
 *
 * É tudo SVG em um viewBox de 200×200, então escala sem perder nitidez — o
 * tamanho real vem da classe (`h-* w-*`) de quem usa.
 */
export function JarvisCore({ label, isActive = true, className }: JarvisCoreProps) {
  const spin = (animation: string) => cn('hud-rotor', isActive && animation)

  return (
    <svg viewBox="0 0 200 200" className={cn('overflow-visible', className)} aria-hidden="true">
      <defs>
        <radialGradient id="jarvis-core-glow">
          <stop offset="0%" stopColor="var(--color-accent)" stopOpacity="0.35" />
          <stop offset="70%" stopColor="var(--color-accent)" stopOpacity="0.06" />
          <stop offset="100%" stopColor="var(--color-accent)" stopOpacity="0" />
        </radialGradient>
      </defs>

      <circle cx="100" cy="100" r="86" fill="url(#jarvis-core-glow)" />

      {/* Traços cardeais, alinhados aos colchetes. */}
      <g stroke="currentColor" strokeWidth="1.2" opacity="0.5">
        <path d="M100 14v9M100 177v9M14 100h9M177 100h9" />
      </g>

      {/* Anel pontilhado externo. */}
      <circle
        cx="100"
        cy="100"
        r="92"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeDasharray="1 6"
        opacity="0.3"
        className={spin('animate-hud-slow')}
      />

      {/* Coroa de marcações. */}
      <circle
        cx="100"
        cy="100"
        r="80"
        fill="none"
        stroke="currentColor"
        strokeWidth="3"
        strokeDasharray="2 14"
        opacity="0.4"
        className={spin('animate-hud-medium')}
      />

      {/* Dois arcos opostos, a camada que mais chama atenção no movimento. */}
      <circle
        cx="100"
        cy="100"
        r="70"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeDasharray="58 161.9"
        opacity="0.75"
        className={cn(spin('animate-hud-fast'), 'hud-glow')}
      />

      {/* Anel principal. */}
      <circle
        cx="100"
        cy="100"
        r="56"
        fill="none"
        stroke="currentColor"
        strokeWidth="1"
        opacity="0.55"
      />
      <circle
        cx="100"
        cy="100"
        r="56"
        fill="none"
        stroke="currentColor"
        strokeWidth="2.2"
        strokeLinecap="round"
        strokeDasharray="96 255.8"
        className={cn(spin('animate-hud-medium'), 'hud-glow')}
      />

      {/* Disco central: o fundo escuro que faz o texto ter contraste. */}
      <circle cx="100" cy="100" r="46" fill="var(--color-base)" opacity="0.85" />
      <circle
        cx="100"
        cy="100"
        r="46"
        fill="none"
        stroke="currentColor"
        strokeWidth="0.8"
        opacity="0.4"
      />

      {/* `textLength` fixa a largura: nomes curtos esticam, longos comprimem, e o
          wordmark nunca vaza do disco — o nome do assistente é configurável. */}
      <text
        x="100"
        y="106"
        textAnchor="middle"
        textLength="72"
        lengthAdjust="spacingAndGlyphs"
        className="fill-content text-[16px] font-light uppercase"
      >
        {label}
      </text>
    </svg>
  )
}
