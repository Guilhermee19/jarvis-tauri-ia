'use client'

import { useEffect, useRef, useState } from 'react'

import { descontar, type Caixa } from '@/lib/buraco'
import { browserExternal } from '@/lib/tauri'
import { cn } from '@/lib/utils'
import { abaAtiva, useJanelaStore, useNavegadorStore } from '@/stores'

/**
 * Tudo que é desenhado ACIMA deste buraco: janelinhas à frente, e a gaveta.
 *
 * Lido do DOM, e não da store, porque o que importa é o retângulo que está na tela agora —
 * a store guarda a posição escolhida, que é `null` enquanto a janelinha nunca foi
 * arrastada, e não sabe o tamanho que o conteúdo deu a ela. O `z-index` é o mesmo que o
 * `zDaJanela` calculou e a `FloatingPanel` escreveu no `style`.
 *
 * **A GAVETA entra sem comparar `z-index`, e não é atalho.** O `Sheet` é `z-40` por
 * construção, acima da faixa 30–39 inteira das janelinhas — o comentário dele explica que
 * é exatamente para uma janela por cima de outra nunca passar por cima da gaveta. Ela está
 * sempre à frente, então não há o que comparar.
 *
 * Sem isto a página ficava desenhada POR CIMA das Configurações: o webview é uma camada
 * nativa e nenhum `z-index` alcança ele, então quem tem que sair da frente é ele.
 */
function porCimaDe(elemento: HTMLElement): Caixa[] {
  const minha = Number(elemento.closest<HTMLElement>('.floating-panel')?.style.zIndex ?? 0)

  // O `sheet-overlay` cobre a área de conteúdo inteira, então uma gaveta aberta some com a
  // página — que é o certo: ela escurece tudo o que está atrás, e uma página acesa por
  // cima do escurecido é justamente o defeito. O `sheet-panel` entra junto como rede: se
  // um dia a gaveta abrir sem overlay, o retângulo dela ainda é descontado.
  const acima = [
    ...document.querySelectorAll<HTMLElement>('.floating-panel, .sheet-overlay, .sheet-panel'),
  ].filter(
    (elemento) =>
      !elemento.classList.contains('floating-panel') || Number(elemento.style.zIndex || 0) > minha,
  )

  return acima.map((alvo) => {
    const caixa = alvo.getBoundingClientRect()
    return {
      x: Math.round(caixa.x),
      y: Math.round(caixa.y),
      largura: Math.round(caixa.width),
      altura: Math.round(caixa.height),
    }
  })
}

/**
 * O navegador interno: barra de abas, barra de endereço, e um buraco.
 *
 * **O buraco é o ponto.** A página não é desenhada por este componente — cada aba é um
 * webview NATIVO, empilhado acima de todo o HTML da janela. O que este arquivo faz é
 * medir o retângulo vazio e contar para o Rust onde encaixá-lo.
 *
 * Duas consequências que explicam quase todo o código daqui:
 *
 * - Nenhum CSS alcança a página. Nada de sombra, borda arredondada ou transparência sobre
 *   ela — o que se vê ali é uma janela do sistema fingindo estar dentro do painel.
 * - Ela cobre o que estiver por baixo. Por isso o webview é recortado para caber só onde
 *   nenhuma janelinha mais alta está — quem faz essa conta é o `lib/buraco`.
 *
 * **A regra antiga era pela ORDEM, e agora é pelo LUGAR.** Antes, qualquer janelinha à
 * frente sumia com a página inteira, mesmo estando do outro lado da tela e sem cobrir um
 * pixel dela. Hoje o que esconde é sobreposição de verdade — e mesmo ela só esconde o que
 * for coberto: uma conversa encostando na borda esquerda do navegador encolhe a página,
 * não a apaga.
 */
export function NavegadorPanel({ escondido = false }: { escondido?: boolean }) {
  const abas = useNavegadorStore((state) => state.abas)
  const ativa = useNavegadorStore((state) => state.ativa)
  const erro = useNavegadorStore((state) => state.erro)
  const abrindo = useNavegadorStore((state) => state.abrindo)
  const selecionar = useNavegadorStore((state) => state.selecionar)
  const fechar = useNavegadorStore((state) => state.fechar)
  const navegar = useNavegadorStore((state) => state.navegar)
  const andar = useNavegadorStore((state) => state.andar)
  const posicionar = useNavegadorStore((state) => state.posicionar)
  const abrirSite = useNavegadorStore((state) => state.abrirSite)
  // Seletor, e não `getState()` dentro do render: o endereço muda por EVENTO quando a
  // pessoa clica num link, e um retrato lido no render não reagiria a isso — a barra
  // mostraria para sempre o endereço com que a aba nasceu.
  const urlAtiva = useNavegadorStore((state) => abaAtiva(state)?.url ?? '')

  // A posição e o tamanho de QUALQUER janelinha mudam a cada quadro enquanto ela é
  // arrastada, e é isso que dispara a remedição — sem um laço de animação girando à toa.
  // São todas, e não só a do navegador, porque agora o recorte depende de onde as outras
  // estão: arrastar a conversa por cima da página tem que encolher a página.
  const arranjos = useJanelaStore((state) => state.arranjos)
  const abertas = useJanelaStore((state) => state.abertas)
  // Abrir a gaveta não mexe em `arranjos` nem em `abertas`, então sem ela nas dependências
  // a remedição nunca rodava — e a geometria certa não adianta se ninguém a recalcula.
  const gaveta = useJanelaStore((state) => state.gaveta)

  const buraco = useRef<HTMLDivElement>(null)
  // Coberta de tal jeito que não sobrou página para mostrar. Guardado porque é o que a
  // mensagem do buraco vazio explica — e ele não dá para deduzir do `abertas`.
  const [coberto, setCoberto] = useState(false)

  useEffect(() => {
    const medir = () => {
      const alvo = buraco.current
      if (!alvo || escondido) {
        posicionar(null)
        return
      }

      const caixa = alvo.getBoundingClientRect()
      // `getBoundingClientRect` já é relativo à área de conteúdo da janela, que é
      // exatamente o sistema de coordenadas que o Tauri usa para posicionar um webview
      // filho. É por isso que não há conversão nenhuma aqui.
      const area = {
        x: Math.round(caixa.x),
        y: Math.round(caixa.y),
        largura: Math.round(caixa.width),
        altura: Math.round(caixa.height),
      }

      const livre = descontar(area, porCimaDe(alvo))
      setCoberto(livre === null)
      posicionar(livre)
    }

    medir()

    // O observador pega o redimensionamento do painel; o `resize` da janela pega o do
    // app inteiro, que não muda o tamanho do buraco mas muda a posição dele.
    const observador = new ResizeObserver(medir)
    if (buraco.current) observador.observe(buraco.current)
    window.addEventListener('resize', medir)

    return () => {
      observador.disconnect()
      window.removeEventListener('resize', medir)
    }
  }, [posicionar, arranjos, abertas, escondido, gaveta])

  // Fechar o painel desmonta este componente, e o webview precisa sumir junto — ele não
  // sabe que o painel dele deixou de existir.
  useEffect(() => () => posicionar(null), [posicionar])

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <Linguetas
        abas={abas}
        ativa={ativa}
        onSelecionar={(id) => void selecionar(id)}
        onFechar={(id) => void fechar(id)}
        onNova={() => void abrirSite('google.com')}
      />

      {ativa ? (
        <BarraDeEndereco
          key={ativa}
          url={urlAtiva}
          onIr={(url) => void navegar(ativa, url)}
          onAndar={(passo) => void andar(ativa, passo)}
          onFora={() => void browserExternal(ativa)}
        />
      ) : null}

      {erro ? (
        <p
          role="alert"
          className="border-danger/30 bg-danger/10 text-danger m-2 rounded-md border px-2.5 py-2 text-[11px] leading-relaxed"
        >
          {erro}
        </p>
      ) : null}

      {/* O buraco. Ele é medido, não preenchido — quem desenha aqui é o sistema.
          A faixa logo abaixo é o que deixa esta janelinha ser redimensionável. */}
      <div ref={buraco} className="bg-base min-h-0 flex-1">
        {abas.length === 0 ? (
          <div className="text-muted flex h-full flex-col items-center justify-center gap-1 px-6 text-center">
            <p className="text-[11px] tracking-[0.18em] uppercase">
              {abrindo ? 'abrindo…' : 'nenhuma aba'}
            </p>
            <p className="text-muted/70 text-[10px] leading-relaxed">
              Peça “abre o youtube” ou “pesquisa preço do dólar” — e a página aparece aqui.
            </p>
          </div>
        ) : coberto ? (
          <div className="text-muted/70 flex h-full items-center justify-center px-6 text-center text-[10px] leading-relaxed">
            Outra janelinha está bem em cima da página, e não sobrou espaço para ela — a página é
            uma camada do sistema e cobriria a janelinha que está por cima. Arraste uma das duas
            para o lado e ela volta.
          </div>
        ) : null}
      </div>

      {/* Faixa reservada para a alça de redimensionar, que fica no canto de baixo à
          direita e tem 16 px (`h-4`).

          Sem ela o buraco iria até a borda, e o webview — camada NATIVA acima de todo o
          HTML — cobriria a alça. O sintoma é específico e enganoso: a janelinha do
          navegador arrasta pelo cabeçalho como as outras (o cabeçalho fica acima do
          webview) mas é a única que não redimensiona, e não há nada de errado com o
          código de redimensionamento. */}
      <div className="h-4 shrink-0" />
    </div>
  )
}

function Linguetas({
  abas,
  ativa,
  onSelecionar,
  onFechar,
  onNova,
}: {
  abas: { id: string; titulo: string }[]
  ativa: string | null
  onSelecionar: (id: string) => void
  onFechar: (id: string) => void
  onNova: () => void
}) {
  return (
    <div className="border-border-soft scroll-thin flex shrink-0 items-center gap-1 overflow-x-auto border-b px-2 py-1.5">
      {abas.map((aba) => (
        <div
          key={aba.id}
          className={cn(
            'flex shrink-0 items-center gap-1.5 rounded border px-2 py-1 text-[10px]',
            aba.id === ativa
              ? 'border-accent/40 bg-accent/10 text-accent'
              : 'border-border-soft text-muted hover:text-content',
          )}
        >
          <button type="button" onClick={() => onSelecionar(aba.id)} className="max-w-32 truncate">
            {aba.titulo}
          </button>
          <button
            type="button"
            onClick={() => onFechar(aba.id)}
            aria-label={`Fechar ${aba.titulo}`}
            className="hover:text-danger shrink-0"
          >
            ✕
          </button>
        </div>
      ))}

      <button
        type="button"
        onClick={onNova}
        aria-label="Abrir uma aba"
        title="Abrir uma aba"
        className="border-border-soft text-muted hover:text-content shrink-0 rounded border px-2 py-1 text-[10px]"
      >
        +
      </button>
    </div>
  )
}

function BarraDeEndereco({
  url,
  onIr,
  onAndar,
  onFora,
}: {
  url: string
  onIr: (url: string) => void
  onAndar: (passo: number) => void
  onFora: () => void
}) {
  // Estado local: quem digita é o dono do campo até apertar Enter.
  const [texto, setTexto] = useState(url)
  const [urlAnterior, setUrlAnterior] = useState(url)

  // ...mas navegar TROCA o campo. Sem isto, clicar num link deixaria a barra mostrando o
  // endereço anterior, que é pior que perder o que estava sendo digitado — e digitar
  // enquanto a página navega sozinha é raro, ao contrário de clicar em link.
  //
  // Ajuste DURANTE o render, e não num efeito: é o padrão que o React documenta para
  // estado derivado de prop, e o único que não custa um render descartado a cada
  // navegação. Um `useEffect` aqui é justamente o que o eslint acusa de cascata.
  if (url !== urlAnterior) {
    setUrlAnterior(url)
    setTexto(url)
  }

  return (
    <div className="border-border-soft flex shrink-0 items-center gap-1 border-b px-2 py-1.5">
      <Botao rotulo="Voltar" onClick={() => onAndar(-1)}>
        ←
      </Botao>
      <Botao rotulo="Avançar" onClick={() => onAndar(1)}>
        →
      </Botao>

      <input
        value={texto}
        onChange={(evento) => setTexto(evento.target.value)}
        onKeyDown={(evento) => {
          if (evento.key === 'Enter') onIr(texto)
        }}
        spellCheck={false}
        aria-label="Endereço"
        className="border-border-soft bg-base text-content focus:border-accent min-w-0 flex-1 rounded border px-2 py-1 font-mono text-[10px] outline-none"
      />

      <Botao rotulo="Abrir no navegador do sistema" onClick={onFora}>
        ↗
      </Botao>
    </div>
  )
}

function Botao({
  rotulo,
  onClick,
  children,
}: {
  rotulo: string
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={rotulo}
      aria-label={rotulo}
      className="border-border-soft text-muted hover:text-content flex h-5 w-5 shrink-0 items-center justify-center rounded border text-[10px]"
    >
      {children}
    </button>
  )
}
