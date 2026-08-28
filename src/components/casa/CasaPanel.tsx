'use client'

import { useEffect, useState } from 'react'

import { ControlesDoAparelho } from './ControlesDoAparelho'
import { DetalhesDoAparelho, ehControleRemoto } from './DetalhesDoAparelho'
import { familiaDoAparelho, IconeDoAparelho } from './IconeDoAparelho'
import {
  EyeIcon,
  EyeOffIcon,
  HouseIcon,
  PowerIcon,
  SettingsIcon,
  SyncIcon,
} from '@/components/ui/icons'
import { cn } from '@/lib/utils'
import { useCasaStore } from '@/stores'
import type { Aparelho } from '@/types'

/**
 * O que existe na sua casa, descoberto ouvindo a rede.
 *
 * A rede diz quem está ligado agora e em que IP. Ela **nunca** diz o nome que você deu no
 * app nem a chave de controle — essas duas vêm da nuvem da Tuya, e a importação acontece
 * sozinha junto com a varredura.
 *
 * A lista se atualiza a cada 30 s enquanto o painel está aberto, então o botão de
 * sincronizar é um ATALHO, não a única forma de achar algo. Por isso ele é um ícone e não
 * ocupa uma linha inteira de texto.
 */
export function CasaPanel() {
  const aparelhos = useCasaStore((state) => state.aparelhos)
  const procurando = useCasaStore((state) => state.procurando)
  const buscouEm = useCasaStore((state) => state.buscouEm)
  const ignorados = useCasaStore((state) => state.ignorados)
  const erro = useCasaStore((state) => state.erro)
  const procurar = useCasaStore((state) => state.procurar)
  const importando = useCasaStore((state) => state.importando)
  const importados = useCasaStore((state) => state.importados)
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
  // Dois motivos diferentes para não ter nome, e só um deles a importação resolve. O
  // `desconhecido@` é um aparelho cujo anúncio não abriu: ele não tem id de Tuya, então a
  // nuvem nunca vai saber quem é. Os outros existem lá e podem estar sem nome LÁ também.
  const semNome = aparelhos.filter(
    (aparelho) => aparelho.nome === null && !aparelho.id.startsWith('desconhecido@'),
  ).length

  const visiveis = aparelhos.filter((aparelho) => !aparelho.oculto)
  const ocultos = aparelhos.filter((aparelho) => aparelho.oculto)
  const presentes = visiveis.filter((aparelho) => aparelho.presente).length

  return (
    <div className="scroll-thin flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-3 py-3">
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={() => void procurar()}
          disabled={procurando}
          aria-label="Procurar aparelhos agora"
          title="Procurar aparelhos agora"
          className={cn(
            'border-border-soft text-muted hover:text-content flex h-7 w-7 shrink-0 items-center justify-center rounded-md border',
            'active:scale-[0.94] disabled:opacity-60 motion-safe:transition-transform',
          )}
        >
          {/* Girar é o segundo canal do "estou trabalhando"; o primeiro é o botão
              desabilitado, que quem tem `prefers-reduced-motion` continua enxergando. */}
          <SyncIcon className={cn('h-4 w-4', procurando && 'motion-safe:animate-spin')} />
        </button>

        <p className="text-muted min-w-0 flex-1 text-[11px] leading-snug">
          {procurando
            ? 'Ouvindo a rede… leva alguns segundos, porque eles se anunciam sozinhos e não dá para pedir que falem antes da hora.'
            : buscouEm === null
              ? 'Procurando pela primeira vez.'
              : `${presentes} de ${visiveis.length} ${visiveis.length === 1 ? 'aparelho respondeu' : 'aparelhos responderam'}. Atualiza sozinho a cada 30 s.`}
        </p>
      </div>

      {/* A importação acontece sozinha junto com a varredura, então isto é um atalho para
          os dois casos em que ela NÃO acontece: quando falhou por alguma configuração da
          Tuya (e aí ela para de tentar sozinha, para não repetir o mesmo erro a cada
          30 s), e quando você pareia um aparelho de novo — o que TROCA a chave dele sem
          que nada na tela mude. */}
      {podeImportar ? (
        <button
          type="button"
          onClick={() => void importar()}
          disabled={importando}
          className="text-muted hover:text-content self-start text-[11px] underline underline-offset-2 disabled:opacity-50"
        >
          {importando
            ? 'Falando com a Tuya…'
            : semNome > 0
              ? `Importar nome e chave de ${semNome === 1 ? 'um aparelho' : `${semNome} aparelhos`}`
              : 'Reimportar nomes e chaves da nuvem'}
        </button>
      ) : null}

      {/* Sem isto, importar não dá sinal nenhum quando não há nome novo para trazer — e
          um botão que parece não fazer nada é indistinguível de um botão quebrado. */}
      {importados !== null && !importando ? (
        <Nota>
          {importados} {importados === 1 ? 'aparelho lido' : 'aparelhos lidos'} da nuvem.
          {semNome > 0
            ? ` ${semNome === 1 ? 'Um continua' : `${semNome} continuam`} sem nome porque também não ${semNome === 1 ? 'tem' : 'têm'} nome na Tuya — renomeie no app Smart Life e importe de novo.`
            : ' Nomes e chaves em dia.'}
        </Nota>
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
      ) : visiveis.length === 0 && ocultos.length === 0 ? (
        <Vazio titulo="Ninguém se anunciou">
          Confira se os aparelhos estão ligados na tomada e se este PC está na{' '}
          <strong className="text-content font-normal">mesma rede Wi-Fi</strong> que eles — muitos
          roteadores separam 2,4 GHz e 5 GHz em redes diferentes, e a maioria das lâmpadas só fala
          2,4 GHz. Se o Windows perguntou sobre o firewall, precisa ter sido liberado.
        </Vazio>
      ) : (
        <ul className="flex flex-col gap-2">
          {visiveis.map((aparelho) => (
            <Card key={aparelho.id} aparelho={aparelho} />
          ))}
        </ul>
      )}

      {ocultos.length > 0 ? <Ocultos aparelhos={ocultos} /> : null}

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

/**
 * As famílias que têm o que mostrar além do liga-desliga.
 *
 * Fora daqui o painel de ajustes abriria vazio, e um botão que leva a lugar nenhum é pior
 * que a ausência dele.
 */
const COM_AJUSTES: string[] = ['lampada', 'sensor', 'tomada', 'interruptor', 'ventilador']

function Card({ aparelho }: { aparelho: Aparelho }) {
  const estado = useCasaStore((state) => state.estados[aparelho.id])
  const comandando = useCasaStore((state) => state.comandando === aparelho.id)
  const alternar = useCasaStore((state) => state.alternar)
  const detalhe = useCasaStore((state) => state.detalhes[aparelho.id])
  // Um painel por vez: abrir os ajustes fecha a ficha técnica. Os dois juntos
  // transformariam um cartão de uma linha numa tela inteira.
  const [painel, setPainel] = useState<'ajustes' | 'ficha' | null>(null)

  // Três condições independentes, e as três precisam valer: saber falar o protocolo
  // dele, ter a chave dele, e ele ser o tipo de coisa que liga e desliga. Faltando
  // qualquer uma, os detalhes explicam QUAL — um botão que sempre dá erro seria pior que
  // a ausência dele.
  const dachaComandar = aparelho.suportado && aparelho.temChave && aparelho.comutavel

  // Controle de infravermelho não está na rede POR PROJETO — ele é uma lista de códigos
  // dentro do emissor. Chamá-lo de "fora do ar" mandaria procurar um problema que não
  // existe, e por isso ele sai de toda a conversa sobre presença.
  // A categoria basta, e é o que salva antes de a importação ligar o controle ao
  // emissor: sem isso a TV apareceria como um aparelho de rede que sumiu.
  // Os dois tipos que **não estão na rede por projeto**: o controle de infravermelho,
  // que é uma lista de códigos dentro do emissor, e o subaparelho ZigBee, que fala pelo
  // gateway. Chamar qualquer um dos dois de "fora do ar" manda procurar um problema que
  // não existe.
  const foraDaRede = ehControleRemoto(aparelho) || aparelho.subaparelho
  const porInfravermelho = ehControleRemoto(aparelho)

  // O botão de ajustes só aparece quando esperamos que haja o que ajustar, e a aposta é
  // feita SEM perguntar ao aparelho: descobrir de verdade custa uma conexão por cartão, e
  // dez conexões para decidir se um ícone aparece seria caro demais.
  //
  // Controle de infravermelho tem teclas; lâmpada tem brilho e cor; sensor tem o que
  // mede; tomada e interruptor podem ter mais de uma chave. Central, câmera e o que ainda
  // não tem categoria ficam de fora — neles o liga-desliga do cartão é tudo o que há.
  //
  // A aposta erra às vezes: uma tomada simples abre e diz que não há mais nada. Errar
  // para o lado de oferecer é melhor que esconder o painel de quem tem o que ajustar.
  const familia = familiaDoAparelho(aparelho.categoria)
  const temAjustes = porInfravermelho || (aparelho.temChave && COM_AJUSTES.includes(familia))

  // A leitura principal no próprio cartão, quando já foi buscada: num sensor de porta
  // "aberta" é o conteúdo inteiro, e escondê-la atrás de um clique anula o sensor.
  // Só em sensor: numa tomada a primeira leitura é um `DP 14 = off` de configuração, e
  // pô-lo no cartão trocaria a informação principal por ruído.
  const leitura = familia === 'sensor' ? detalhe?.leituras[0] : undefined

  // O que o cartão fechado precisa gritar, e nada além. O resto — id, endereço, modelo,
  // data points — mora atrás do "i", porque só interessa quando algo não funciona.
  const alerta = foraDaRede
    ? null
    : !aparelho.presente
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
        !foraDaRede && (!aparelho.suportado || !aparelho.presente) && 'opacity-70',
      )}
    >
      <div className="flex items-center gap-2">
        <Status
          ativo={aparelho.ativo}
          suportado={aparelho.suportado}
          presente={aparelho.presente || foraDaRede}
          porInfravermelho={foraDaRede}
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

        {leitura ? (
          <span className="text-accent shrink-0 text-[11px]" title={leitura.rotulo}>
            {leitura.valor}
          </span>
        ) : null}

        {alerta ? <Etiqueta alerta={alerta === 'sem chave'}>{alerta}</Etiqueta> : null}

        {dachaComandar ? (
          <Interruptor
            ligado={estado}
            ocupado={comandando}
            onAlternar={(ligado) => void alternar(aparelho, ligado)}
          />
        ) : null}

        {temAjustes ? (
          <button
            type="button"
            onClick={() => setPainel(painel === 'ajustes' ? null : 'ajustes')}
            aria-expanded={painel === 'ajustes'}
            aria-label={painel === 'ajustes' ? 'Fechar os ajustes' : 'Ajustar este aparelho'}
            title="Ajustes"
            className={cn(
              'border-border-soft flex h-5 w-5 shrink-0 items-center justify-center rounded-full border',
              painel === 'ajustes'
                ? 'border-accent/40 bg-accent/15 text-accent'
                : 'text-muted hover:text-content',
            )}
          >
            <SettingsIcon className="h-3 w-3" />
          </button>
        ) : null}

        <button
          type="button"
          onClick={() => setPainel(painel === 'ficha' ? null : 'ficha')}
          aria-expanded={painel === 'ficha'}
          aria-label={painel === 'ficha' ? 'Fechar a ficha técnica' : 'Ver a ficha técnica'}
          title="Ficha técnica"
          className={cn(
            'border-border-soft flex h-5 w-5 shrink-0 items-center justify-center rounded-full border font-serif text-[10px] italic',
            painel === 'ficha'
              ? 'border-accent/40 bg-accent/15 text-accent'
              : 'text-muted hover:text-content',
          )}
        >
          i
        </button>
      </div>

      {painel === 'ajustes' ? <ControlesDoAparelho aparelho={aparelho} /> : null}
      {painel === 'ficha' ? <DetalhesDoAparelho aparelho={aparelho} /> : null}

      {/* Fora do painel de detalhes de propósito: é o que explica por que NÃO há botão,
          e uma explicação escondida atrás de um clique não seria encontrada por quem
          está justamente procurando o botão que falta. */}
      {painel === 'ficha' && !dachaComandar && !foraDaRede ? (
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
 * A gaveta do que você tirou da lista.
 *
 * Fechada por padrão e no fim da tela: o ponto de ocultar é não ver. Mas ela **existe** e
 * diz quantos são — um aparelho que some sem deixar rastro vira meia hora procurando por
 * que ele não aparece mais.
 */
function Ocultos({ aparelhos }: { aparelhos: Aparelho[] }) {
  const [aberto, setAberto] = useState(false)
  const ocultar = useCasaStore((state) => state.ocultar)

  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        onClick={() => setAberto(!aberto)}
        aria-expanded={aberto}
        className="text-muted hover:text-content flex items-center gap-1.5 self-start text-[11px]"
      >
        <EyeOffIcon className="h-3.5 w-3.5" />
        {aparelhos.length} {aparelhos.length === 1 ? 'aparelho oculto' : 'aparelhos ocultos'}
      </button>

      {aberto ? (
        <ul className="flex flex-col gap-1">
          {aparelhos.map((aparelho) => (
            <li
              key={aparelho.id}
              className="border-border-soft flex items-center gap-2 rounded-md border border-dashed px-3 py-1.5 opacity-70"
            >
              <IconeDoAparelho categoria={aparelho.categoria} className="text-muted shrink-0" />
              <span className="text-muted min-w-0 flex-1 truncate text-[11px]">
                {aparelho.nome || aparelho.ip || aparelho.id}
              </span>
              <button
                type="button"
                onClick={() => void ocultar(aparelho, false)}
                title="Trazer de volta para a lista"
                aria-label={`Trazer ${aparelho.nome || aparelho.ip} de volta para a lista`}
                className="text-muted hover:text-content shrink-0"
              >
                <EyeIcon className="h-3.5 w-3.5" />
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  )
}

/**
 * Um botão de energia, e não um interruptor deslizante.
 *
 * O símbolo já diz o que ele faz sem rótulo, e é o mesmo desenho de qualquer aparelho —
 * ninguém precisa aprender esta tela. A COR é o estado: aceso quando ligado, apagado
 * quando desligado, e neutro quando ninguém perguntou ao aparelho ainda, que é a verdade
 * até alguém abrir os detalhes.
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
      aria-label={ligado ? 'Desligar' : 'Ligar'}
      className={cn(
        'flex h-6 w-6 shrink-0 items-center justify-center rounded-full border',
        'active:scale-[0.92] disabled:opacity-50 motion-safe:transition-transform',
        ligado
          ? 'border-accent/50 bg-accent/15 text-accent hud-glow'
          : 'border-border-soft text-muted hover:text-content',
      )}
    >
      <PowerIcon className="h-3.5 w-3.5" />
    </button>
  )
}

/** Um ponto, quatro significados. Cor é o canal, mas o `title` é o que garante a leitura. */
function Status({
  ativo,
  suportado,
  presente,
  porInfravermelho,
}: {
  ativo: boolean
  suportado: boolean
  presente: boolean
  porInfravermelho?: boolean
}) {
  // Ausente vem primeiro: não adianta dizer que ele é controlável se ele não está lá.
  const [cor, texto] = porInfravermelho
    ? ['bg-accent/60', 'Comandado por infravermelho — não está na rede, e não precisa estar']
    : !presente
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
