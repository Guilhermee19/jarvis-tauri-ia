'use client'

import { useEffect, useRef, useState } from 'react'

import { browserExternal } from '@/lib/tauri'
import { cn } from '@/lib/utils'
import { abaAtiva, useJanelaStore, useNavegadorStore } from '@/stores'

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
 * - Ela cobre o que estiver por baixo. Por isso as abas só aparecem quando este painel é
 *   o da FRENTE: sem essa regra, abrir a conversa por cima do navegador desenharia a
 *   conversa embaixo dele.
 */
export function NavegadorPanel() {
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

  // A posição e o tamanho do painel mudam a cada quadro enquanto ele é arrastado, e é
  // isso que dispara a remedição — sem um laço de animação girando à toa.
  const arranjo = useJanelaStore((state) => state.arranjos.navegador)
  const abertas = useJanelaStore((state) => state.abertas)
  const naFrente = abertas[abertas.length - 1] === 'navegador'

  const buraco = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const medir = () => {
      const alvo = buraco.current
      if (!alvo || !naFrente) {
        posicionar(null)
        return
      }

      const caixa = alvo.getBoundingClientRect()
      posicionar({
        // `getBoundingClientRect` já é relativo à área de conteúdo da janela, que é
        // exatamente o sistema de coordenadas que o Tauri usa para posicionar um webview
        // filho. É por isso que não há conversão nenhuma aqui.
        x: Math.round(caixa.x),
        y: Math.round(caixa.y),
        largura: Math.round(caixa.width),
        altura: Math.round(caixa.height),
      })
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
  }, [posicionar, naFrente, arranjo])

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
          url={abaAtiva(useNavegadorStore.getState())?.url ?? ''}
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

      {/* O buraco. Ele é medido, não preenchido — quem desenha aqui é o sistema. */}
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
        ) : !naFrente ? (
          <div className="text-muted/70 flex h-full items-center justify-center px-6 text-center text-[10px] leading-relaxed">
            A página fica escondida enquanto outra janelinha está na frente — ela é uma camada do
            sistema e cobriria o que estivesse por cima. Clique aqui para trazê-la de volta.
          </div>
        ) : null}
      </div>
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
  // Estado local e não controlado pela store: quem digita é o dono do campo até apertar
  // Enter. Refletir a URL de volta a cada navegação apagaria o que estivesse sendo escrito.
  const [texto, setTexto] = useState(url)

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
