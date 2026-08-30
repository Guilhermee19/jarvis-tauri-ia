/**
 * Ícones inline em SVG.
 *
 * Sem biblioteca de ícones: `currentColor` já resolve o tema.
 *
 * **A conta passou de 15**, que era o limite combinado para trocar por `lucide-react` de
 * uma vez. Fica como dívida real: o próximo ícone que entrar aqui é o gatilho, e não mais
 * um "quando der".
 *
 * O gatilho FOI puxado pelo {@link VigilanciaIcon}, e a troca não aconteceu junto de
 * propósito: ela mexe em todos os componentes que importam daqui, e misturá-la com a
 * feature de câmeras faria um diff onde nenhuma das duas coisas dá para revisar. É a
 * próxima tarefa deste arquivo, sozinha.
 */

interface IconProps {
  className?: string
}

function Svg({
  className,
  children,
  // Traçado é o padrão da casa; os de mídia invertem para preenchido.
  fill = 'none',
  stroke = 'currentColor',
}: IconProps & { children: React.ReactNode; fill?: string; stroke?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      // Um `viewBox` sozinho NÃO dá tamanho ao SVG: sem `width`/`height` e sem classe
      // de tamanho, o ícone colapsa e o botão fica visivelmente vazio — foi o que
      // aconteceu com o microfone do chat, que era o único uso sem `h-*`/`w-*`.
      // `1em` acompanha a fonte do botão, e como atributo de apresentação PERDE para
      // qualquer classe do Tailwind: quem passa `h-4 w-4` continua mandando, e quem
      // esquecer ganha um ícone visível em vez de um buraco.
      width="1em"
      height="1em"
      fill={fill}
      stroke={stroke}
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

/* Os três de mídia são preenchidos, e não traçados: nesse tamanho um triângulo em
   contorno some, e o pictograma de player é sólido em todo lugar. */

export function PlayIcon(props: IconProps) {
  return (
    <Svg {...props} fill="currentColor" stroke="none">
      <path d="M8 5.4v13.2l10.5-6.6z" />
    </Svg>
  )
}

export function PauseIcon(props: IconProps) {
  return (
    <Svg {...props} fill="currentColor" stroke="none">
      <rect x="7" y="5" width="3.6" height="14" rx="1" />
      <rect x="13.4" y="5" width="3.6" height="14" rx="1" />
    </Svg>
  )
}

export function PrevIcon(props: IconProps) {
  return (
    <Svg {...props} fill="currentColor" stroke="none">
      <rect x="5" y="5.5" width="2.4" height="13" rx="1" />
      <path d="M19 5.9v12.2L9.2 12z" />
    </Svg>
  )
}

export function NextIcon(props: IconProps) {
  return (
    <Svg {...props} fill="currentColor" stroke="none">
      <path d="M5 5.9v12.2L14.8 12z" />
      <rect x="16.6" y="5.5" width="2.4" height="13" rx="1" />
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

/**
 * Conversa por voz: ondas saindo dos dois lados.
 *
 * Deliberadamente NÃO é o microfone com um enfeite — os dois botões ficam lado a
 * lado no chat e fazem coisas diferentes (um dita uma frase, o outro abre um
 * diálogo). Ícones parecidos ali seriam duas portas iguais para salas diferentes.
 */
export function ConversationIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M12 8v8" />
      <path d="M8.5 6.5v11M15.5 6.5v11" />
      <path d="M5 9.5v5M19 9.5v5" />
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

/**
 * Câmera de vigilância — a da casa, montada na parede.
 *
 * Desenho deliberadamente diferente do {@link CameraIcon}, que é a webcam: os dois
 * botões ficam na mesma barra, e dois pictogramas parecidos ali fariam o usuário abrir a
 * webcam querendo ver a garagem. O corpo inclinado e o suporte são o que se lê de
 * relance como "câmera de segurança".
 */
export function VigilanciaIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M3.4 9.1 14.6 6.1a1 1 0 0 1 1.23.71l.8 3a1 1 0 0 1-.71 1.22l-11.2 3a1 1 0 0 1-1.23-.71l-.8-3a1 1 0 0 1 .71-1.22Z" />
      <path d="m17.2 8.5 3.3-1.5v5.2l-3.3-1.5" />
      <path d="M8.4 13.9 10 19.4" />
      <path d="M7 20h6" />
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

/** A casa: telhado e porta. Os aparelhos inteligentes vivem atrás dele. */
export function HouseIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M3.5 10.5 12 4l8.5 6.5V20h-17Z" />
      <path d="M9.8 20v-5.2h4.4V20" />
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

export function SyncIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M20 11.5a8 8 0 0 0-13.7-5.2L3.5 9" />
      <path d="M4 12.5a8 8 0 0 0 13.7 5.2l2.8-2.7" />
      <path d="M3.5 4.5V9H8" />
      <path d="M20.5 19.5V15H16" />
    </Svg>
  )
}

export function EyeIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12Z" />
      <circle cx="12" cy="12" r="2.8" />
    </Svg>
  )
}

export function EyeOffIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M9.9 5.8A9.6 9.6 0 0 1 12 5.5c6 0 9.5 6.5 9.5 6.5a17 17 0 0 1-2.7 3.6" />
      <path d="M6.2 7.4A17 17 0 0 0 2.5 12S6 18.5 12 18.5c1.5 0 2.8-.4 4-1" />
      <path d="M10.1 10.1a2.8 2.8 0 0 0 3.8 3.8" />
      <path d="M3.5 3.5l17 17" />
    </Svg>
  )
}

export function GaugeIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M4.5 19.5v-5" />
      <path d="M9.5 19.5v-9" />
      <path d="M14.5 19.5v-13" />
      <path d="M19.5 19.5v-6.5" />
    </Svg>
  )
}

export function GlobeIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M3.5 12h17" />
      <path d="M12 3.5a13 13 0 0 1 0 17a13 13 0 0 1 0-17Z" />
    </Svg>
  )
}

/** Nós ligados — o mapa do conhecimento. */
export function GrafoIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <circle cx="5" cy="6" r="2" />
      <circle cx="19" cy="8" r="2" />
      <circle cx="12" cy="17" r="2.5" />
      <path d="M6.8 7.2 10.4 15" />
      <path d="M17.4 9.5 13.6 15.3" />
      <path d="M7 6.4h10" />
    </Svg>
  )
}
