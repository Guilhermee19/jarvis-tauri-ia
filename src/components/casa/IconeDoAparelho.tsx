/**
 * O desenho que diz **que tipo de coisa** é cada aparelho da casa.
 *
 * Arquivo próprio, e não o `ui/icons.tsx`: aquele guarda o vocabulário da interface
 * (chat, câmera, engrenagem) e o comentário no topo dele pede para migrar para uma
 * biblioteca depois de ~15. Estes aqui são outra família — eles crescem junto com as
 * categorias da Tuya, não com as telas do app.
 *
 * A categoria vem da nuvem, no `Conhecido.categoria`. **Antes de importar ela é vazia**,
 * e é por isso que o desconhecido não é um erro: é o estado normal de quem só foi ouvido
 * na rede. Nesse caso o ícone genérico é a resposta honesta — sabemos que há algo ali, e
 * não sabemos o quê.
 */

interface Props {
  categoria: string
  className?: string
}

function Svg({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      // Mesmo motivo do `ui/icons.tsx`: sem `width`/`height` o SVG colapsa, e `1em`
      // perde para qualquer classe de tamanho do Tailwind.
      width="1em"
      height="1em"
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

/**
 * De categoria da Tuya para um dos desenhos.
 *
 * As chaves são os códigos crus dela — `dj` é lâmpada, `cz` é tomada, `wg2` é gateway.
 * Parecem arbitrários porque são: vêm do catálogo da Tuya, e traduzi-los para um enum
 * nosso só acrescentaria uma tabela a manter no meio do caminho.
 */
const POR_CATEGORIA: Record<string, Familia> = {
  dj: 'lampada',
  xdd: 'lampada',
  fwd: 'lampada',
  dc: 'lampada',
  dd: 'lampada',
  gyd: 'lampada',
  cz: 'tomada',
  pc: 'tomada',
  kg: 'interruptor',
  tgq: 'interruptor',
  tgkg: 'interruptor',
  tdq: 'interruptor',
  fs: 'ventilador',
  fsd: 'ventilador',
  qt: 'controle',
  wnykq: 'controle',
  ykq: 'controle',
  infrared_ac: 'controle',
  infrared_tv: 'controle',
  wg2: 'hub',
  sp: 'camera',
  mcs: 'sensor',
  mcs2: 'sensor',
  pir: 'sensor',
  hps: 'sensor',
  ywbj: 'sensor',
  rqbj: 'sensor',
  sj: 'sensor',
  wsdcg: 'sensor',
}

type Familia =
  | 'lampada'
  | 'tomada'
  | 'interruptor'
  | 'ventilador'
  | 'controle'
  | 'hub'
  | 'camera'
  | 'sensor'
  | 'generico'

/** O nome legível de cada família, para o `title` e o leitor de tela. */
const NOME: Record<Familia, string> = {
  lampada: 'Lâmpada',
  tomada: 'Tomada',
  interruptor: 'Interruptor',
  ventilador: 'Ventilador',
  controle: 'Controle infravermelho',
  hub: 'Central',
  camera: 'Câmera',
  sensor: 'Sensor',
  generico: 'Tipo ainda desconhecido',
}

/**
 * A família de um aparelho, pela categoria da Tuya.
 *
 * Exportada porque não serve só para escolher o desenho: quem decide se o cartão ganha
 * botão de ajustes precisa saber se aquilo é uma lâmpada, e ler a mesma tabela é o que
 * impede as duas respostas de divergirem.
 *
 * Categorias com sufixo (`infrared_ac`) e variações de caixa aparecem no catálogo da
 * Tuya; normalizar aqui evita uma entrada por variante na tabela.
 */
export function familiaDoAparelho(categoria: string): Familia {
  return POR_CATEGORIA[categoria.trim().toLowerCase()] ?? 'generico'
}

export function IconeDoAparelho({ categoria, className }: Props) {
  const familia = familiaDoAparelho(categoria)

  return (
    <span title={NOME[familia]} aria-label={NOME[familia]} role="img" className={className}>
      {DESENHO[familia]}
    </span>
  )
}

const DESENHO: Record<Familia, React.ReactNode> = {
  lampada: (
    <Svg>
      <path d="M9 17.5a5.5 5.5 0 1 1 6 0v1.5H9v-1.5Z" />
      <path d="M10 21.5h4" />
    </Svg>
  ),
  tomada: (
    <Svg>
      <rect x="4.5" y="4.5" width="15" height="15" rx="3.5" />
      <circle cx="9.5" cy="10.5" r="1" />
      <circle cx="14.5" cy="10.5" r="1" />
      <path d="M9.5 15.5h5" />
    </Svg>
  ),
  interruptor: (
    <Svg>
      <rect x="5" y="3.5" width="14" height="17" rx="2.5" />
      <path d="M9 9h6" />
      <path d="M9 14.5h6" />
    </Svg>
  ),
  ventilador: (
    <Svg>
      <circle cx="12" cy="12" r="2" />
      <path d="M12 10c0-3 1-5 3.5-5S18 8.5 14 10.8" />
      <path d="M14 12c3 0 5 1 5 3.5S15.5 18 13.2 14" />
      <path d="M10 14c0 3-1 5-3.5 5S6 15.5 10 13.2" />
    </Svg>
  ),
  controle: (
    <Svg>
      <rect x="7.5" y="2.5" width="9" height="19" rx="3" />
      <path d="M10.5 6.5h3" />
      <circle cx="10.5" cy="11.5" r="0.9" />
      <circle cx="13.5" cy="11.5" r="0.9" />
      <circle cx="10.5" cy="15.5" r="0.9" />
      <circle cx="13.5" cy="15.5" r="0.9" />
    </Svg>
  ),
  hub: (
    <Svg>
      <rect x="3" y="13" width="18" height="7.5" rx="2" />
      <path d="M6.5 16.8h.01" />
      <path d="M12 9.5a4.5 4.5 0 0 1 4.5-4.5" />
      <path d="M12 9.5a4.5 4.5 0 0 0-4.5-4.5" />
    </Svg>
  ),
  camera: (
    <Svg>
      <rect x="3" y="6.5" width="13" height="11" rx="2.5" />
      <path d="M16 11.5l5-3v10l-5-3v-4Z" />
    </Svg>
  ),
  sensor: (
    <Svg>
      <circle cx="12" cy="12" r="2.5" />
      <path d="M7.5 7.5a6.4 6.4 0 0 0 0 9" />
      <path d="M16.5 7.5a6.4 6.4 0 0 1 0 9" />
    </Svg>
  ),
  generico: (
    <Svg>
      <rect x="4.5" y="4.5" width="15" height="15" rx="3.5" />
      <path d="M12 15.5v.01" />
      <path d="M12 12.8c0-1.6 1.8-1.7 1.8-3.1A1.8 1.8 0 0 0 10.3 9" />
    </Svg>
  ),
}
