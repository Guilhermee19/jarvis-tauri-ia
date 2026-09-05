'use client'

import type { CeuId } from '@/types'

/**
 * Os desenhos do céu, em SVG inline.
 *
 * **Não moram no `ui/icons.tsx`, e isso é deliberado.** Aquele arquivo abre dizendo que a
 * conta de ícones passou do combinado, que o gatilho para trocar por `lucide-react` já foi
 * puxado, e que a migração é "a próxima tarefa daquele arquivo, sozinha". Empilhar seis
 * ícones novos lá dentro tornaria aquele diff ainda maior e misturaria duas coisas que
 * precisam ser revisadas separadas.
 *
 * Aqui eles são um conjunto fechado e de domínio — céu, não interface —, mapeado 1:1 com
 * o que a `lucide-react` chama de `Sun`, `CloudSun`, `Cloud`, `CloudFog`, `CloudRain`,
 * `CloudLightning` e `Snowflake`. Quando a migração acontecer, este arquivo vira sete
 * imports e some.
 *
 * `currentColor` no traço e `1em` no tamanho pelo mesmo motivo do outro arquivo: quem
 * decide cor e tamanho é o texto ao redor, então o tema já vem resolvido.
 */

interface CeuProps {
  className?: string
}

function Svg({ children, className }: CeuProps & { children: React.ReactNode }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="1em"
      height="1em"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
    >
      {children}
    </svg>
  )
}

/** A nuvem que serve de base para chuva, trovoada e neve — desenhada uma vez só. */
const NUVEM = <path d="M17.5 18a4.5 4.5 0 0 0-.9-8.9A6 6 0 0 0 5.2 10.4 3.8 3.8 0 0 0 6 18Z" />

/**
 * Exportado porque a barra de baixo também precisa dele.
 *
 * É o botão do card, e ele mora aqui em vez de no `ui/icons.tsx` pela mesma razão dos
 * outros seis: aquele arquivo está fechado para ícone novo até a migração acontecer.
 */
export function Sol({ className }: CeuProps) {
  return (
    <Svg className={className}>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
    </Svg>
  )
}

function SolComNuvem({ className }: CeuProps) {
  return (
    <Svg className={className}>
      <path d="M8 5.5V4M4.2 7.2 3.1 6.1M4 11H2.5M11.9 7.2 13 6.1" />
      <circle cx="8" cy="10" r="2.6" />
      <path d="M17.5 19a4 4 0 0 0-.8-7.9A5.4 5.4 0 0 0 6.4 12.3 3.4 3.4 0 0 0 7 19Z" />
    </Svg>
  )
}

function Nuvem({ className }: CeuProps) {
  return <Svg className={className}>{NUVEM}</Svg>
}

function Nevoa({ className }: CeuProps) {
  return (
    <Svg className={className}>
      <path d="M16.5 15a4 4 0 0 0-.8-7.9A5.4 5.4 0 0 0 5.4 8.3 3.4 3.4 0 0 0 6 15" />
      <path d="M4 18.5h11M8 21.5h9" />
    </Svg>
  )
}

function Chuva({ className }: CeuProps) {
  return (
    <Svg className={className}>
      <path d="M16.5 14a4 4 0 0 0-.8-7.9A5.4 5.4 0 0 0 5.4 7.3 3.4 3.4 0 0 0 6 14" />
      <path d="M8 17.5 7 20M12 17.5 11 20M16 17.5 15 20" />
    </Svg>
  )
}

function Trovoada({ className }: CeuProps) {
  return (
    <Svg className={className}>
      <path d="M16.5 13.5a4 4 0 0 0-.8-7.9A5.4 5.4 0 0 0 5.4 6.8 3.4 3.4 0 0 0 6 13.5" />
      <path d="M13 15.5l-3.5 4h3l-1 3.5 4-4.5h-3z" />
    </Svg>
  )
}

function Neve({ className }: CeuProps) {
  return (
    <Svg className={className}>
      <path d="M16.5 14a4 4 0 0 0-.8-7.9A5.4 5.4 0 0 0 5.4 7.3 3.4 3.4 0 0 0 6 14" />
      <path d="M8 18.5h.01M12 20.5h.01M16 18.5h.01M10 21.5h.01M14 17.5h.01" />
    </Svg>
  )
}

const POR_CEU: Record<CeuId, (props: CeuProps) => React.ReactElement> = {
  limpo: Sol,
  'poucas-nuvens': SolComNuvem,
  nublado: Nuvem,
  nevoa: Nevoa,
  chuva: Chuva,
  trovoada: Trovoada,
  neve: Neve,
}

/** O desenho do céu para uma das famílias do [`CeuId`]. */
export function IconeDoCeu({ ceu, className }: { ceu: CeuId; className?: string }) {
  const Desenho = POR_CEU[ceu]
  return <Desenho className={className} />
}
