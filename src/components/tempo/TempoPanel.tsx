'use client'

import { IconeDoCeu } from './icones'
import { useTempoStore } from '@/stores'
import { cn } from '@/lib/utils'
import { ceuDoCodigo, type DiaDeTempo } from '@/types'

/**
 * Como chamar um dia da previsão.
 *
 * "Hoje" e "Amanhã" são o que a pessoa procura primeiro; do terceiro em diante o nome do
 * dia da semana localiza melhor que uma data. A comparação é com o relógio de QUEM ESTÁ
 * LENDO, e a data veio no fuso do LUGAR — pedir o tempo em Tóquio de madrugada pode
 * rotular como "Amanhã" o dia que lá já começou. É a aproximação certa: o rótulo serve
 * para orientar quem lê, não para descrever o fuso de lá.
 */
function rotuloDoDia(data: string, indice: number): string {
  if (indice === 0) return 'Hoje'
  if (indice === 1) return 'Amanhã'

  const [ano, mes, dia] = data.split('-').map(Number)
  if (!ano || !mes || !dia) return data

  const quando = new Date(ano, mes - 1, dia)
  const nome = quando.toLocaleDateString('pt-BR', { weekday: 'short' })

  // "seg." vira "Seg" — o ponto some porque a coluna é estreita e a maiúscula alinha com
  // "Hoje" e "Amanhã" ao lado.
  const limpo = nome.replace('.', '')
  return limpo.charAt(0).toUpperCase() + limpo.slice(1)
}

/** Uma coluna da semana. */
function Coluna({ dia, indice }: { dia: DiaDeTempo; indice: number }) {
  const ceu = ceuDoCodigo(dia.ceu)

  return (
    <div
      className={cn(
        'flex min-w-0 flex-col items-center gap-1 rounded-md px-1 py-2',
        // Hoje ganha fundo em vez de borda: borda somaria uma linha vertical no meio de
        // uma fileira que já é toda feita de colunas estreitas.
        indice === 0 && 'bg-surface-hover',
      )}
    >
      <span className="text-muted text-[10px] tracking-[0.08em] uppercase">
        {rotuloDoDia(dia.data, indice)}
      </span>

      <IconeDoCeu ceu={ceu.id} className="text-accent text-xl" />

      <span className="text-content text-[11px] tabular-nums">
        {Math.round(dia.maxima)}°<span className="text-muted">/{Math.round(dia.minima)}°</span>
      </span>

      {/* Só quando é relevante — o mesmo corte de 30% que a frase falada usa, para a tela
          não afirmar "0%" onde a voz não disse nada. */}
      {dia.chuva >= 30 ? (
        <span className="text-accent text-[10px] tabular-nums">{dia.chuva}%</span>
      ) : (
        <span className="text-[10px] opacity-0">—</span>
      )}
    </div>
  )
}

/**
 * O card do tempo: agora em cima, a semana embaixo.
 *
 * Os números são os MESMOS que ele acabou de falar — ver `AcaoDeUi::Tempo`. A fala usa
 * três dias e este card usa todos os sete: quem corta a fala é a `Previsao::frase`, porque
 * uma semana lida em voz alta é insuportável, e olhar a semana é para o que serve um card.
 */
export function TempoPanel() {
  const previsao = useTempoStore((state) => state.previsao)
  const lugar = useTempoStore((state) => state.lugar)
  const atualizado = useTempoStore((state) => state.atualizado)

  if (previsao === null) {
    return (
      <div className="text-muted flex flex-1 items-center justify-center p-6 text-center text-xs">
        Pergunte “como está o tempo?” e a previsão aparece aqui.
      </div>
    )
  }

  const agora = ceuDoCodigo(previsao.ceu)

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-3">
      <div className="flex items-center gap-3">
        <IconeDoCeu ceu={agora.id} className="text-accent text-4xl" />

        <div className="flex min-w-0 flex-col">
          <span className="text-content text-2xl leading-none tabular-nums">
            {Math.round(previsao.temperatura)}°
          </span>
          <span className="text-content truncate text-xs">{agora.rotulo}</span>
          <span className="text-muted truncate text-[10px]">
            {lugar === '' ? 'Aqui' : lugar} · umidade {previsao.umidade}%
          </span>
        </div>
      </div>

      {/* `grid-cols-7` fixo e não `flex`: colunas de larguras diferentes fariam os ícones
          dançarem de posição entre um dia e outro, e é justamente a fileira de ícones que
          se lê de relance. */}
      <div className="grid grid-cols-7 gap-0.5">
        {previsao.dias.map((dia, indice) => (
          <Coluna key={dia.data} dia={dia} indice={indice} />
        ))}
      </div>

      {atualizado !== null && (
        <span className="text-muted/60 mt-auto text-[10px]">
          Consultado às{' '}
          {new Date(atualizado).toLocaleTimeString('pt-BR', {
            hour: '2-digit',
            minute: '2-digit',
          })}{' '}
          · Open-Meteo
        </span>
      )}
    </div>
  )
}
