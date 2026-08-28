'use client'

import { useEffect } from 'react'

import { cn } from '@/lib/utils'
import { useCasaStore } from '@/stores'
import type { Aparelho, Luz, Tecla } from '@/types'

/**
 * O que dá para MEXER num aparelho: as teclas de um controle, o brilho e a cor de uma
 * lâmpada.
 *
 * Separado dos detalhes técnicos de propósito. São coisas de natureza diferente e com
 * frequências de uso opostas: mexer na cor é o uso diário, e olhar o identificador e os
 * data points acontece uma vez, quando algo não funciona. Misturar os dois obrigava a
 * passar pelo que não se quer para chegar ao que se quer.
 *
 * O conteúdo é buscado na ABERTURA — perguntar a dez aparelhos o que eles aceitam, para
 * desenhar controles que ninguém pediu, custaria uma conexão por cartão.
 */
export function ControlesDoAparelho({ aparelho }: { aparelho: Aparelho }) {
  const detalhe = useCasaStore((state) => state.detalhes[aparelho.id])
  const ocupado = useCasaStore((state) => state.detalhando === aparelho.id)
  const detalhar = useCasaStore((state) => state.detalhar)
  const ajustarLuz = useCasaStore((state) => state.ajustarLuz)
  const controle = useCasaStore((state) => state.controles[aparelho.id])
  const carregarTeclas = useCasaStore((state) => state.carregarTeclas)
  const apertarTecla = useCasaStore((state) => state.apertarTecla)

  // Dois caminhos, porque são dois tipos de aparelho. O de rede responde ele mesmo o que
  // aceita; o controle de infravermelho não tem endereço, e quem sabe as teclas dele é a
  // nuvem.
  //
  // **Só quem tem protocolo é perguntado.** Sem versão, o aparelho nunca se anunciou, e
  // conectar daria o erro "não sei falar o protocolo ''" — que culpa o protocolo por uma
  // coisa que é a ausência de rede.
  useEffect(() => {
    if (aparelho.emissor) void carregarTeclas(aparelho)
    else if (detalhe === undefined && aparelho.versao) void detalhar(aparelho)
    // O aparelho muda de identidade só pelo id; as outras propriedades dele mudam a cada
    // varredura e reexecutariam isto sem necessidade.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [aparelho.id])

  return (
    <div className="border-border-soft mt-2.5 flex flex-col gap-3 border-t pt-2.5 pl-4">
      {aparelho.emissor ? (
        <Teclado
          teclas={controle?.teclas}
          ocupado={ocupado}
          onApertar={(tecla) => void apertarTecla(aparelho, tecla)}
        />
      ) : null}

      {detalhe?.luz && aparelho.temChave ? (
        <ControlesDaLuz
          luz={detalhe.luz}
          ocupado={ocupado}
          onAjustar={(ajuste) => void ajustarLuz(aparelho, ajuste)}
        />
      ) : null}

      {/* Nem sempre há o que mexer, e o botão de ajustes só aparece quando esperamos que
          haja. Quando a expectativa erra, dizer isso é melhor que um painel vazio. */}
      {!aparelho.emissor && !detalhe?.luz ? (
        <p className="text-muted text-[10px] leading-relaxed">
          {ocupado
            ? 'perguntando ao aparelho o que ele aceita…'
            : 'Este aparelho não respondeu nenhum ajuste além do liga-desliga.'}
        </p>
      ) : null}
    </div>
  )
}

/**
 * As teclas de um controle de infravermelho.
 *
 * **Sem estado, e é assim mesmo.** O emissor pisca um LED e não tem como saber se a TV
 * obedeceu — não há retorno, não há confirmação, e desenhar um "ligado" aqui seria
 * inventar informação. É o mesmo contrato do controle de plástico na mesa.
 *
 * A ordem é a que a Tuya devolve, que é a do controle original: `Power` na frente, e
 * depois volume, canal e navegação. Reordenar por conta própria só faria procurar.
 */
function Teclado({
  teclas,
  ocupado,
  onApertar,
}: {
  teclas: Tecla[] | undefined
  ocupado: boolean
  onApertar: (tecla: Tecla) => void
}) {
  if (teclas === undefined) {
    return <p className="text-muted text-[10px]">perguntando à nuvem quais teclas ele tem…</p>
  }

  if (teclas.length === 0) {
    return (
      <p className="text-muted text-[10px] leading-relaxed">
        Este controle não tem tecla nenhuma cadastrada. Configure-o no app Smart Life e importe de
        novo.
      </p>
    )
  }

  return (
    <div className="flex flex-wrap gap-1">
      {teclas.map((tecla) => (
        <button
          key={`${tecla.keyId}-${tecla.key}`}
          type="button"
          disabled={ocupado}
          onClick={() => onApertar(tecla)}
          title={tecla.key}
          className={cn(
            'border-border-soft bg-surface text-muted hover:text-content rounded border px-2 py-1 text-[10px]',
            'active:scale-[0.94] disabled:opacity-50 motion-safe:transition-transform',
          )}
        >
          {tecla.keyName || tecla.key}
        </button>
      ))}
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
