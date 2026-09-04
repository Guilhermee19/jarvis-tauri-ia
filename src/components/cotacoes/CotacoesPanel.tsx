'use client'

import { cn } from '@/lib/utils'
import { useCotacoesStore } from '@/stores'
import { NOME_DA_MOEDA, type Cotacao } from '@/types'

/**
 * O card de cotações: dólar, euro, bitcoin e ethereum, com a variação do dia.
 *
 * **Não busca nada.** Os números chegam do agente pelo `AcaoDeUi::Cotacoes`, no mesmo
 * turno em que ele responde por voz — é isso que garante que a tela e a fala mostrem o
 * mesmo instante. O porquê inteiro está no `cotacoesStore`.
 *
 * Por consequência o card é um RETRATO, não um painel ao vivo, e o rodapé diz a hora da
 * cotação por causa disso: número sem idade parece tempo real.
 */
export function CotacoesPanel() {
  const cotacoes = useCotacoesStore((state) => state.cotacoes)
  const atualizado = useCotacoesStore((state) => state.atualizado)

  if (cotacoes.length === 0) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-2 px-6 text-center">
        <p className="text-content text-sm font-medium">Nenhuma cotação ainda.</p>
        <p className="text-muted text-xs">
          Pergunte &ldquo;quanto tá o dólar?&rdquo; ou &ldquo;como estão as moedas?&rdquo; e elas
          aparecem aqui.
        </p>
      </div>
    )
  }

  return (
    <div className="scroll-thin flex flex-1 flex-col gap-2 overflow-y-auto p-3">
      {cotacoes.map((cotacao) => (
        <LinhaDaMoeda key={cotacao.codigo} cotacao={cotacao} />
      ))}

      {atualizado ? (
        <p className="text-muted pt-1 text-center text-[10px]">
          cotado em {horaDe(atualizado)} · AwesomeAPI
        </p>
      ) : null}
    </div>
  )
}

function LinhaDaMoeda({ cotacao }: { cotacao: Cotacao }) {
  // O zero tem lado, e a escolha importa: "estável" em verde sugere alta que não houve.
  // Abaixo de 0,05% o número arredondaria para 0,00% de qualquer jeito — é a mesma régua
  // do `Cotacao::movimento` no Rust, e as duas precisam concordar.
  const parado = Math.abs(cotacao.variacao) < 0.05
  const subiu = cotacao.variacao > 0

  return (
    <div className="border-border-soft bg-surface/50 flex items-center gap-3 rounded-lg border px-3 py-2.5">
      <div className="min-w-0 flex-1">
        <div className="text-content text-sm font-medium">
          {NOME_DA_MOEDA[cotacao.codigo] ?? cotacao.codigo}
        </div>
        {/* `tabular-nums` para a mínima e a máxima não dançarem entre uma abertura e
            outra — quatro linhas com dígitos de larguras diferentes viram serrilha. */}
        <div className="text-muted text-[10px] tabular-nums">
          {reais(cotacao.minima)} — {reais(cotacao.maxima)}
        </div>
      </div>

      <div className="text-right">
        <div className="text-content text-sm font-semibold tabular-nums">
          {reais(cotacao.valor)}
        </div>
        <div
          className={cn(
            'text-[10px] font-medium tabular-nums',
            parado ? 'text-muted' : subiu ? 'text-emerald-400' : 'text-red-400',
          )}
        >
          {parado ? '—' : `${subiu ? '+' : ''}${cotacao.variacao.toFixed(2).replace('.', ',')}%`}
        </div>
      </div>
    </div>
  )
}

/**
 * As casas seguem a GRANDEZA, não a moeda — é a mesma regra do `Cotacao::formatado` no
 * Rust, porque o card e a fala não podem mostrar números diferentes para o mesmo preço.
 * Centavo importa em 5 reais e é ruído em 413 mil.
 */
function reais(valor: number): string {
  return valor.toLocaleString('pt-BR', {
    style: 'currency',
    currency: 'BRL',
    maximumFractionDigits: valor >= 1000 ? 0 : 2,
  })
}

/** `2026-09-03 22:40:57` → `22:40`. Corta a string em vez de construir um `Date`: o
 *  carimbo já vem no fuso de Brasília, e `new Date` o reinterpretaria como local. */
function horaDe(quando: string): string {
  return quando.slice(11, 16) || quando
}
