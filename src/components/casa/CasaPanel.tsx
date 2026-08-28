'use client'

import { useEffect, useState } from 'react'

import { Button } from '@/components/ui/Button'
import { DetalhesDoAparelho } from './DetalhesDoAparelho'
import { IconeDoAparelho } from './IconeDoAparelho'
import { HouseIcon } from '@/components/ui/icons'
import { cn } from '@/lib/utils'
import { useCasaStore } from '@/stores'
import type { Aparelho } from '@/types'

/**
 * O que existe na sua casa, descoberto ouvindo a rede.
 *
 * A rede diz quem está ligado agora e em que IP. Ela **nunca** diz o nome que você deu no
 * app nem a chave de controle — essas duas vêm da nuvem da Tuya, uma vez, pelo botão de
 * importar. O painel mostra os dois estados na cara, porque uma lista de ids sem botão
 * nenhum e sem explicação parece defeito.
 */
export function CasaPanel() {
  const aparelhos = useCasaStore((state) => state.aparelhos)
  const procurando = useCasaStore((state) => state.procurando)
  const buscouEm = useCasaStore((state) => state.buscouEm)
  const ignorados = useCasaStore((state) => state.ignorados)
  const erro = useCasaStore((state) => state.erro)
  const procurar = useCasaStore((state) => state.procurar)
  const importando = useCasaStore((state) => state.importando)
  const importar = useCasaStore((state) => state.importar)
  const iniciarRonda = useCasaStore((state) => state.iniciarRonda)
  const pararRonda = useCasaStore((state) => state.pararRonda)

  // A ronda vive enquanto o painel estiver aberto. Fora dele não há tela para atualizar,
  // e quem precisa do endereço de um aparelho para comandar por voz lê o que ficou
  // anotado em disco — que é justamente o que a ronda mantém em dia.
  useEffect(() => {
    iniciarRonda()

    return pararRonda
  }, [iniciarRonda, pararRonda])

  // Só faz sentido oferecer a importação quando há a quem dar nome: a busca na nuvem
  // parte de um id que a varredura viu.
  const podeImportar = aparelhos.some((aparelho) => !aparelho.id.startsWith('desconhecido@'))
  const semNome = aparelhos.filter((aparelho) => aparelho.nome === null).length
  const presentes = aparelhos.filter((aparelho) => aparelho.presente).length

  return (
    <div className="scroll-thin flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-3 py-3">
      <Button
        onClick={() => void procurar()}
        disabled={procurando}
        // `transition-transform` e não `transition-all`: aqui só a escala se move, e
        // listar a propriedade evita o navegador vigiar tudo que pode mudar.
        className="active:scale-[0.96] motion-safe:transition-transform"
      >
        {procurando ? (
          <>
            {/* O ponto pulsando é o segundo canal: quem tem `prefers-reduced-motion`
                ligado ainda lê "Ouvindo a rede" e vê o botão desabilitado. */}
            <span className="bg-accent h-1.5 w-1.5 shrink-0 rounded-full motion-safe:animate-pulse" />
            Ouvindo a rede…
          </>
        ) : (
          'Procurar aparelhos'
        )}
      </Button>

      {procurando ? (
        <Nota>
          Leva alguns segundos. Eles se anunciam sozinhos de tempos em tempos, e não dá para pedir
          que falem antes da hora.
        </Nota>
      ) : null}

      {/* A importação acontece sozinha junto com a varredura. Este botão é para os dois
          casos em que ela não acontece: quando ainda falhou por configuração da Tuya, e
          quando você pareou um aparelho de novo — o que TROCA a chave dele. */}
      {podeImportar && semNome > 0 ? (
        <div className="flex flex-col gap-1.5">
          <Button
            onClick={() => void importar()}
            disabled={importando}
            className="active:scale-[0.96] motion-safe:transition-transform"
          >
            {importando ? (
              <>
                <span className="bg-accent h-1.5 w-1.5 shrink-0 rounded-full motion-safe:animate-pulse" />
                Falando com a Tuya…
              </>
            ) : (
              'Importar nomes e chaves da nuvem'
            )}
          </Button>
          <Nota>
            {semNome === 1 ? 'Um aparelho está' : `${semNome} aparelhos estão`} sem nome e sem chave
            de controle. As duas coisas vivem na nuvem da Tuya e saem de lá uma vez só — depois
            disso o controle é local, sem internet. Precisa das credenciais em Configurações.
          </Nota>
        </div>
      ) : podeImportar ? (
        <button
          type="button"
          onClick={() => void importar()}
          disabled={importando}
          className="text-muted hover:text-content self-start text-[11px] underline underline-offset-2 disabled:opacity-50"
        >
          {importando ? 'Falando com a Tuya…' : 'Reimportar nomes e chaves da nuvem'}
        </button>
      ) : null}

      {erro ? (
        <p
          role="alert"
          className="border-danger/30 bg-danger/10 text-danger rounded-md border px-2.5 py-2 text-[11px] leading-relaxed"
        >
          {erro}
        </p>
      ) : null}

      {/* Três situações que parecem a mesma tela vazia e têm causas diferentes:
          nunca procurou / procurou e ninguém falou / achou. Confundi-las é o que faz
          alguém achar que o programa quebrou. */}
      {buscouEm === null ? (
        <Vazio titulo="Nada procurado ainda">
          A busca escuta a rede local por alguns segundos. Não precisa de conta, senha nem internet.
        </Vazio>
      ) : aparelhos.length === 0 ? (
        <Vazio titulo="Ninguém se anunciou">
          Confira se os aparelhos estão ligados na tomada e se este PC está na{' '}
          <strong className="text-content font-normal">mesma rede Wi-Fi</strong> que eles — muitos
          roteadores separam 2,4 GHz e 5 GHz em redes diferentes, e a maioria das lâmpadas só fala
          2,4 GHz. Se o Windows perguntou sobre o firewall, precisa ter sido liberado.
        </Vazio>
      ) : (
        <ul className="flex flex-col gap-2">
          {aparelhos.map((aparelho) => (
            <Card key={aparelho.id} aparelho={aparelho} />
          ))}
        </ul>
      )}

      {/* Quantos dos que estão na lista responderam agora. Sem isto, "3 aparelhos" leria
          como "3 aparelhos ligados", e dois deles podem estar fora da tomada. */}
      {buscouEm !== null && aparelhos.length > 0 ? (
        <Nota>
          {presentes} de {aparelhos.length} {aparelhos.length === 1 ? 'aparelho' : 'aparelhos'}{' '}
          {presentes === 1 ? 'respondeu' : 'responderam'} na última varredura. A lista se atualiza
          sozinha a cada 30 segundos enquanto este painel estiver aberto.
        </Nota>
      ) : null}

      {/* Só depois de uma busca, e só quando houve o que ignorar. É a diferença entre
          "ninguém falou" e "falaram e eu não entendi" — duas telas idênticas com
          soluções opostas. */}
      {buscouEm !== null && ignorados > 0 ? (
        <Nota>
          {ignorados} {ignorados === 1 ? 'pacote chegou' : 'pacotes chegaram'} na rede sem serem
          reconhecidos. Pode ser outro protocolo passando pela mesma porta, ou um aparelho que fala
          um formato que o Jarvis ainda não lê.
        </Nota>
      ) : null}
    </div>
  )
}

function Card({ aparelho }: { aparelho: Aparelho }) {
  const estado = useCasaStore((state) => state.estados[aparelho.id])
  const comandando = useCasaStore((state) => state.comandando === aparelho.id)
  const alternar = useCasaStore((state) => state.alternar)
  const [aberto, setAberto] = useState(false)

  // Três condições independentes, e as três precisam valer: saber falar o protocolo
  // dele, ter a chave dele, e ele ser o tipo de coisa que liga e desliga. Faltando
  // qualquer uma, os detalhes explicam QUAL — um botão que sempre dá erro seria pior que
  // a ausência dele.
  const dachaComandar = aparelho.suportado && aparelho.temChave && aparelho.comutavel

  // O que o cartão fechado precisa gritar, e nada além. O resto — id, endereço, modelo,
  // data points — mora atrás do "i", porque só interessa quando algo não funciona.
  const alerta = !aparelho.presente
    ? 'fora do ar'
    : !aparelho.decifrado
      ? 'anúncio não lido'
      : !aparelho.suportado
        ? 'sem controle'
        : aparelho.nome && !aparelho.temChave
          ? 'sem chave'
          : null

  return (
    // Raio menor que o do painel: a janelinha é `rounded-lg` e estes cartões vivem
    // dentro dela com folga — repetir o mesmo raio faz o cartão parecer espremido.
    <li
      className={cn(
        'border-border-soft bg-base/40 rounded-md border px-3 py-2',
        (!aparelho.suportado || !aparelho.presente) && 'opacity-70',
      )}
    >
      <div className="flex items-center gap-2">
        <Status
          ativo={aparelho.ativo}
          suportado={aparelho.suportado}
          presente={aparelho.presente}
        />

        {/* O tipo antes do nome: numa lista de dez, o ícone é o que deixa achar a
            lâmpada sem ler. Vazio antes de importar, e aí o desenho é o genérico. */}
        <IconeDoAparelho categoria={aparelho.categoria} className="text-muted shrink-0 text-sm" />

        {/* O nome quando ele existe; senão o IP, que é o que dá para conferir no
            roteador. O id nunca aparece aqui: ele não diz nada a um humano. */}
        <span
          className={cn(
            'text-content min-w-0 flex-1 truncate text-xs',
            !aparelho.nome && 'font-mono tabular-nums',
          )}
        >
          {aparelho.nome || aparelho.ip || aparelho.id}
        </span>

        {alerta ? <Etiqueta alerta={alerta === 'sem chave'}>{alerta}</Etiqueta> : null}

        {dachaComandar ? (
          <Interruptor
            ligado={estado}
            ocupado={comandando}
            onAlternar={(ligado) => void alternar(aparelho, ligado)}
          />
        ) : null}

        <button
          type="button"
          onClick={() => setAberto(!aberto)}
          aria-expanded={aberto}
          aria-label={aberto ? 'Fechar detalhes' : 'Ver detalhes'}
          title={aberto ? 'Fechar detalhes' : 'Ver detalhes'}
          className={cn(
            'border-border-soft flex h-5 w-5 shrink-0 items-center justify-center rounded-full border text-[10px] font-serif italic',
            aberto ? 'border-accent/40 bg-accent/15 text-accent' : 'text-muted hover:text-content',
          )}
        >
          i
        </button>
      </div>

      {aberto ? <DetalhesDoAparelho aparelho={aparelho} /> : null}

      {/* Fora do painel de detalhes de propósito: é o que explica por que NÃO há botão,
          e uma explicação escondida atrás de um clique não seria encontrada por quem
          está justamente procurando o botão que falta. */}
      {aberto && !dachaComandar ? (
        <p className="text-muted mt-2 pl-4 text-[10px] leading-relaxed">
          {!aparelho.decifrado
            ? 'Anunciou num protocolo que não abriu. Fica na lista com o endereço para você saber que ele existe, em vez de sumir e parecer problema de rede.'
            : !aparelho.suportado
              ? `O Jarvis lê o anúncio dele, mas ainda não sabe mandar comando no protocolo ${aparelho.versao}.`
              : !aparelho.temChave
                ? 'Falta a chave de controle dele. Ela vem da nuvem da Tuya, no botão de importar.'
                : 'Este tipo de aparelho não tem um liga-desliga. Ele responde e é alcançável, mas o que fazer com ele depende do que está ligado nele — no caso de uma central, dos aparelhos que ela controla.'}
        </p>
      ) : null}
    </li>
  )
}

/**
 * Um interruptor de dois estados, e não dois botões.
 *
 * Dois botões era o certo enquanto o estado só era conhecido depois de alguém mandar um
 * comando. Agora os detalhes perguntam ao aparelho, então há uma posição de partida de
 * verdade — e `undefined` (ninguém perguntou ainda) fica como um terceiro visual, neutro,
 * em vez de mentir que está desligado.
 */
function Interruptor({
  ligado,
  ocupado,
  onAlternar,
}: {
  ligado: boolean | undefined
  ocupado: boolean
  onAlternar: (ligado: boolean) => void
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={ligado ?? false}
      disabled={ocupado}
      onClick={() => onAlternar(!ligado)}
      title={ligado === undefined ? 'Estado ainda não consultado' : ligado ? 'Ligado' : 'Desligado'}
      className={cn(
        'relative h-4 w-7 shrink-0 rounded-full transition-colors disabled:opacity-50',
        ligado ? 'bg-accent' : 'bg-surface-hover',
      )}
    >
      <span
        className={cn(
          'absolute top-0.5 h-3 w-3 rounded-full transition-all',
          ligado ? 'left-3.5 bg-base' : 'bg-muted left-0.5',
          // Ninguém perguntou ainda: o botão fica no meio, que é a verdade.
          ligado === undefined && 'left-2 opacity-60',
        )}
      />
    </button>
  )
}

/** Um ponto, quatro significados. Cor é o canal, mas o `title` é o que garante a leitura. */
function Status({
  ativo,
  suportado,
  presente,
}: {
  ativo: boolean
  suportado: boolean
  presente: boolean
}) {
  // Ausente vem primeiro: não adianta dizer que ele é controlável se ele não está lá.
  const [cor, texto] = !presente
    ? ['bg-muted/30', 'Já apareceu na rede, mas não respondeu na última varredura']
    : !suportado
      ? ['bg-muted/40', 'Encontrado, mas o controle dele ainda não está pronto']
      : ativo
        ? ['bg-accent', 'Pareado e respondendo']
        : ['bg-muted/60', 'Ainda não pareado com nenhum app']

  return (
    <span
      title={texto}
      aria-label={texto}
      className={cn(
        'h-2 w-2 shrink-0 rounded-full',
        cor,
        presente && suportado && ativo && 'hud-glow',
      )}
    />
  )
}

function Etiqueta({ children, alerta }: { children: React.ReactNode; alerta?: boolean }) {
  return (
    <span
      className={cn(
        'shrink-0 rounded-sm border px-1.5 py-0.5 text-[10px] leading-none tracking-[0.06em]',
        alerta
          ? 'border-danger/30 bg-danger/10 text-danger'
          : 'border-border-soft text-muted bg-transparent',
      )}
    >
      {children}
    </span>
  )
}

/**
 * O vazio com nome e explicação, não uma área em branco.
 *
 * O ícone apagado ocupa o lugar do que vai aparecer ali — sem ele o painel parece meio
 * carregado, e não vazio de propósito.
 */
function Vazio({ titulo, children }: { titulo: string; children: React.ReactNode }) {
  return (
    <div className="border-border-soft/60 flex flex-col items-center gap-2 rounded-md border border-dashed px-4 py-6 text-center">
      <HouseIcon className="text-muted/30 h-6 w-6" />
      <p className="text-content text-[11px] tracking-[0.14em] uppercase">{titulo}</p>
      <p className="text-muted max-w-[38ch] text-[11px] leading-relaxed">{children}</p>
    </div>
  )
}

function Nota({ children }: { children: React.ReactNode }) {
  return (
    <p className="text-muted border-border-soft/60 border-l-2 pl-2.5 text-[11px] leading-relaxed">
      {children}
    </p>
  )
}
