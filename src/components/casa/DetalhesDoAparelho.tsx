'use client'

import { useEffect } from 'react'

import { EyeOffIcon } from '@/components/ui/icons'
import { useCasaStore } from '@/stores'
import type { Aparelho } from '@/types'

/**
 * A ficha técnica: identificador, endereço, protocolo, data points crus.
 *
 * **Só interessa quando algo não funciona.** Na tela o tempo todo isso vira ruído que faz
 * a lista inteira parecer complicada, e é por isso que mora atrás do "i" — separado do
 * painel de ajustes, que é o do uso diário.
 *
 * Os data points crus são o item mais valioso daqui: eles revelam um aparelho que faz
 * algo que este app ainda não modela, e são a primeira coisa a olhar quando um comando
 * não pega.
 */
export function DetalhesDoAparelho({ aparelho }: { aparelho: Aparelho }) {
  const detalhe = useCasaStore((state) => state.detalhes[aparelho.id])
  const ocupado = useCasaStore((state) => state.detalhando === aparelho.id)
  const detalhar = useCasaStore((state) => state.detalhar)
  const ocultar = useCasaStore((state) => state.ocultar)

  // **Só quem tem protocolo é perguntado.** Sem versão, o aparelho nunca se anunciou na
  // rede, e conectar nele daria o erro "não sei falar o protocolo ''" — que culpa o
  // protocolo por uma coisa que é a ausência de rede. Controle de infravermelho é o caso
  // normal disso: ele não tem endereço, e não há a quem perguntar.
  useEffect(() => {
    if (detalhe === undefined && !aparelho.emissor && (aparelho.versao || aparelho.subaparelho)) {
      void detalhar(aparelho)
    }
    // O aparelho muda de identidade só pelo id; as outras propriedades dele mudam a cada
    // varredura e reexecutariam isto sem necessidade.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [aparelho.id])

  const remoto = ehControleRemoto(aparelho)

  return (
    <div className="border-border-soft mt-2.5 flex flex-col gap-3 border-t pt-2.5 pl-4">
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-[10px]">
        {/* Controle de infravermelho não tem endereço nem protocolo: ele não está na
            rede. Mostrar "—" em dois campos seria pior que mostrar de quem ele sai. */}
        {remoto ? (
          <Linha rotulo="Emite por">{aparelho.emissor || 'ainda não ligado a um emissor'}</Linha>
        ) : aparelho.subaparelho ? (
          // Subaparelho ZigBee não tem endereço nem protocolo próprios: os do gateway é
          // que valem, e mostrar "—" em dois campos esconderia justamente isso.
          <Linha rotulo="Ligado ao">gateway ZigBee</Linha>
        ) : (
          <>
            <Linha rotulo="Endereço">{aparelho.ip || '—'}</Linha>
            <Linha rotulo="Protocolo">v{aparelho.versao || '—'}</Linha>
          </>
        )}

        <Linha rotulo="Identificador">{aparelho.id}</Linha>
        {aparelho.categoria ? <Linha rotulo="Categoria">{aparelho.categoria}</Linha> : null}
        {aparelho.produto ? <Linha rotulo="Modelo">{aparelho.produto}</Linha> : null}
        <Linha rotulo="Chave">{aparelho.temChave ? 'importada' : 'não importada'}</Linha>

        {remoto || aparelho.subaparelho ? null : (
          <Linha rotulo="Visto">
            {aparelho.presente
              ? 'agora'
              : aparelho.vistoEm > 0
                ? new Date(aparelho.vistoEm).toLocaleString()
                : 'nunca'}
          </Linha>
        )}

        {remoto ? null : (
          <Linha rotulo="Data points">
            {ocupado && !detalhe
              ? 'perguntando ao aparelho…'
              : detalhe
                ? JSON.stringify(detalhe.dps)
                : '—'}
          </Linha>
        )}
      </dl>

      {/* Controle sem emissor: a nuvem sabe as teclas dele, mas ninguém perguntou de QUEM
          ele sai — e essa ligação só é feita na importação. */}
      {remoto && !aparelho.emissor ? (
        <p className="text-muted text-[10px] leading-relaxed">
          As teclas deste controle existem, mas falta saber por qual emissor ele sai — e essa
          ligação é feita na importação. Clique em{' '}
          <strong className="text-content font-normal">Reimportar nomes e chaves da nuvem</strong>,
          no topo do painel, e ele ganha os botões.
        </p>
      ) : null}

      {/* O emissor em si não controla nada: quem tem tecla são os controles que moram
          dentro dele, e cada um deles é um cartão próprio nesta mesma lista. */}
      {ehEmissor(aparelho) ? (
        <p className="text-muted text-[10px] leading-relaxed">
          Este é o emissor: ele aponta o infravermelho, e não tem botão próprio. Quem tem as teclas
          são os controles configurados nele —{' '}
          <strong className="text-content font-normal">TV</strong> e afins aparecem como cartões
          próprios nesta lista, cada um com os botões dele.
        </p>
      ) : null}

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

/**
 * Um controle remoto virtual: TV, ar-condicionado, ventilador de teto.
 *
 * A categoria basta, e é o que salva quando a ligação com o emissor ainda não foi feita:
 * sem isso o cartão tentaria conectar na rede num aparelho que não tem endereço.
 */
export function ehControleRemoto(aparelho: Aparelho): boolean {
  return aparelho.emissor !== '' || aparelho.categoria.startsWith('infrared')
}

/** O aparelho de Wi-Fi que emite o infravermelho — `qt` é o universal, `wnykq` o de ar. */
function ehEmissor(aparelho: Aparelho): boolean {
  return aparelho.categoria === 'qt' || aparelho.categoria === 'wnykq'
}

function Linha({ rotulo, children }: { rotulo: string; children: React.ReactNode }) {
  return (
    <>
      <dt className="text-muted/70">{rotulo}</dt>
      <dd className="text-muted break-all font-mono">{children}</dd>
    </>
  )
}
