'use client'

import { useEffect } from 'react'

import { EyeOffIcon } from '@/components/ui/icons'
import { cn } from '@/lib/utils'
import { useCasaStore } from '@/stores'
import type { Aparelho, Luz } from '@/types'

/**
 * O que fica ESCONDIDO atrás do ícone de informação, num cartão que por fora só mostra
 * nome, tipo e o botão.
 *
 * Duas coisas moram aqui, e é de propósito que sejam as duas. Os **detalhes técnicos**
 * (id, protocolo, data points crus) só interessam quando algo não funciona — na tela o
 * tempo todo eles viram ruído que faz a lista inteira parecer complicada. E os
 * **controles de luz**, que dependem de perguntar ao aparelho o que ele aceita: isso
 * custa uma conexão TCP, e não se paga por dez aparelhos para desenhar controles que
 * ninguém pediu.
 *
 * Por isso o conteúdo é buscado na ABERTURA, e não na varredura.
 */
export function DetalhesDoAparelho({ aparelho }: { aparelho: Aparelho }) {
  const detalhe = useCasaStore((state) => state.detalhes[aparelho.id])
  const ocupado = useCasaStore((state) => state.detalhando === aparelho.id)
  const detalhar = useCasaStore((state) => state.detalhar)
  const ajustarLuz = useCasaStore((state) => state.ajustarLuz)
  const ocultar = useCasaStore((state) => state.ocultar)

  // Busca uma vez ao abrir. Reabrir mostra o que já se sabe na hora e não repete a
  // conexão — quem atualiza de verdade é mexer num controle, que relê no fim.
  useEffect(() => {
    if (detalhe === undefined) void detalhar(aparelho)
    // O aparelho muda de identidade só pelo id; as outras propriedades dele mudam a cada
    // varredura e reexecutariam isto sem necessidade.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [aparelho.id])

  return (
    <div className="border-border-soft mt-2.5 flex flex-col gap-3 border-t pt-2.5 pl-4">
      {detalhe?.luz && aparelho.temChave ? (
        <ControlesDaLuz
          luz={detalhe.luz}
          ocupado={ocupado}
          onAjustar={(ajuste) => void ajustarLuz(aparelho, ajuste)}
        />
      ) : null}

      <Tecnico aparelho={aparelho} dps={detalhe?.dps} carregando={ocupado && !detalhe} />

      {/* Aqui dentro e não no cartão: ocultar é uma decisão que se toma uma vez, e um
          botão de sumir ao lado do de ligar seria clicado por engano. */}
      <button
        type="button"
        onClick={() => void ocultar(aparelho, true)}
        className="text-muted hover:text-content flex items-center gap-1.5 self-start text-[10px]"
      >
        <EyeOffIcon className="h-3.5 w-3.5" />
        Ocultar da lista
      </button>
    </div>
  )
}

function ControlesDaLuz({
  luz,
  ocupado,
  onAjustar,
}: {
  luz: Luz
  ocupado: boolean
  onAjustar: (ajuste: { brilho?: number; temperatura?: number; matiz?: number; saturacao?: number }) => void
}) {
  return (
    <div className="flex flex-col gap-2.5">
      <div className="flex items-center gap-2">
        <span
          className="border-border-soft h-5 w-5 shrink-0 rounded-full border"
          style={{ background: comoCss(luz) }}
          title="A cor que ela está exibindo agora"
        />
        <span className="text-muted text-[10px] uppercase tracking-wide">
          {luz.modo === 'colour' ? 'cor' : luz.modo === 'white' ? 'branco' : luz.modo}
        </span>
      </div>

      {luz.temBrilho ? (
        <Faixa
          rotulo="Brilho"
          valor={luz.brilho}
          min={10}
          max={1000}
          ocupado={ocupado}
          onSolta={(brilho) => onAjustar({ brilho })}
        />
      ) : null}

      {luz.temCor ? (
        <>
          <Faixa
            rotulo="Cor"
            valor={luz.matiz}
            min={0}
            max={360}
            ocupado={ocupado}
            // O fundo é o próprio espectro: um controle de matiz sem ele obriga a
            // arrastar e olhar a lâmpada para descobrir onde está o azul.
            trilha="linear-gradient(to right, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00)"
            onSolta={(matiz) => onAjustar({ matiz, saturacao: luz.saturacao || 1000 })}
          />
          <Faixa
            rotulo="Intensidade"
            valor={luz.saturacao}
            min={0}
            max={1000}
            ocupado={ocupado}
            onSolta={(saturacao) => onAjustar({ matiz: luz.matiz, saturacao })}
          />
        </>
      ) : null}

      {luz.temBranco ? (
        <Faixa
          rotulo="Temperatura"
          valor={luz.temperatura}
          min={0}
          max={1000}
          ocupado={ocupado}
          trilha="linear-gradient(to right, #ffb46b, #fff, #cfe4ff)"
          onSolta={(temperatura) => onAjustar({ temperatura })}
        />
      ) : null}
    </div>
  )
}

/**
 * Um controle deslizante que **só manda ao soltar**.
 *
 * `onChange` dispararia uma conexão TCP por pixel arrastado, e a lâmpada — que responde
 * uma coisa de cada vez — engasgaria e ficaria piscando. O `onMouseUp`/`onTouchEnd` faz
 * um comando por gesto, que é o que a pessoa quis dizer.
 */
function Faixa({
  rotulo,
  valor,
  min,
  max,
  ocupado,
  trilha,
  onSolta,
}: {
  rotulo: string
  valor: number
  min: number
  max: number
  ocupado: boolean
  trilha?: string
  onSolta: (valor: number) => void
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-muted text-[10px] font-medium">{rotulo}</span>
      <input
        type="range"
        min={min}
        max={max}
        defaultValue={valor}
        disabled={ocupado}
        // `key` no valor: quando o aparelho devolve um valor diferente do pedido (ele
        // arredonda), o controle precisa renascer na posição real em vez de mentir.
        key={valor}
        onMouseUp={(evento) => onSolta(Number(evento.currentTarget.value))}
        onTouchEnd={(evento) => onSolta(Number(evento.currentTarget.value))}
        className={cn(
          'h-1.5 w-full cursor-pointer appearance-none rounded-full disabled:opacity-50',
          !trilha && 'bg-surface-hover',
        )}
        style={trilha ? { background: trilha } : undefined}
      />
    </label>
  )
}

function Tecnico({
  aparelho,
  dps,
  carregando,
}: {
  aparelho: Aparelho
  dps: Record<string, unknown> | undefined
  carregando: boolean
}) {
  return (
    <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-[10px]">
      <Linha rotulo="Endereço">{aparelho.ip || '—'}</Linha>
      <Linha rotulo="Protocolo">v{aparelho.versao}</Linha>
      <Linha rotulo="Identificador">{aparelho.id}</Linha>
      {aparelho.categoria ? <Linha rotulo="Categoria">{aparelho.categoria}</Linha> : null}
      {aparelho.produto ? <Linha rotulo="Modelo">{aparelho.produto}</Linha> : null}
      <Linha rotulo="Chave">{aparelho.temChave ? 'importada' : 'não importada'}</Linha>
      <Linha rotulo="Visto">
        {aparelho.presente
          ? 'agora'
          : aparelho.vistoEm > 0
            ? new Date(aparelho.vistoEm).toLocaleString()
            : 'nunca'}
      </Linha>
      {/* Os data points crus: é o que revela um aparelho que faz algo que este app ainda
          não modela, e a primeira coisa a olhar quando um comando não pega. */}
      <Linha rotulo="Data points">
        {carregando ? 'perguntando ao aparelho…' : dps ? JSON.stringify(dps) : '—'}
      </Linha>
    </dl>
  )
}

function Linha({ rotulo, children }: { rotulo: string; children: React.ReactNode }) {
  return (
    <>
      <dt className="text-muted/70">{rotulo}</dt>
      <dd className="text-muted break-all font-mono">{children}</dd>
    </>
  )
}

/**
 * A cor atual da lâmpada, em CSS.
 *
 * A Tuya guarda HSV e o CSS quer HSL — são coisas diferentes, e tratar um como o outro
 * mostra um pastel lavado no lugar de uma cor forte. No modo branco, o que importa é a
 * temperatura, e a matiz guardada é lixo de outro momento.
 */
function comoCss(luz: Luz): string {
  if (luz.modo !== 'colour') {
    // Do âmbar ao azul gelo, acompanhando o mesmo caminho do controle de temperatura.
    const frio = luz.temperatura / 1000
    return `hsl(${30 + frio * 180}, ${45 - frio * 35}%, ${70 + frio * 20}%)`
  }

  const s = luz.saturacao / 1000
  const v = luz.brilho / 1000
  const l = v * (1 - s / 2)
  const sHsl = l === 0 || l === 1 ? 0 : (v - l) / Math.min(l, 1 - l)

  return `hsl(${luz.matiz}, ${Math.round(sHsl * 100)}%, ${Math.round(l * 100)}%)`
}
