'use client'

import { Button } from '@/components/ui/Button'
import { HouseIcon } from '@/components/ui/icons'
import { cn } from '@/lib/utils'
import { useCasaStore } from '@/stores'
import type { Aparelho } from '@/types'

/**
 * O que existe na sua casa, descoberto ouvindo a rede.
 *
 * Esta fase só ENXERGA. Controlar depende da chave de cada aparelho, que vem da nuvem da
 * Tuya — e o painel diz isso na cara, porque uma lista de coisas sem botão nenhum, sem
 * explicação, parece defeito.
 */
export function CasaPanel() {
  const aparelhos = useCasaStore((state) => state.aparelhos)
  const procurando = useCasaStore((state) => state.procurando)
  const buscouEm = useCasaStore((state) => state.buscouEm)
  const ignorados = useCasaStore((state) => state.ignorados)
  const erro = useCasaStore((state) => state.erro)
  const procurar = useCasaStore((state) => state.procurar)

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
  return (
    // Raio menor que o do painel: a janelinha é `rounded-lg` e estes cartões vivem
    // dentro dela com folga — repetir o mesmo raio faz o cartão parecer espremido.
    <li
      className={cn(
        'border-border-soft bg-base/40 rounded-md border px-3 py-2.5',
        // Sem transição de cor no hover: uma lista inteira reagindo ao passar do mouse
        // é ruído, e não há nada para clicar nesta fase.
        !aparelho.suportado && 'opacity-70',
      )}
    >
      <div className="flex items-center gap-2">
        <Status ativo={aparelho.ativo} suportado={aparelho.suportado} />

        {/* O IP na frente porque é o que dá para conferir no roteador; o id é o
            identificador de verdade, mas não diz nada a um humano. O nome ("Luz
            Cozinha") vive na nuvem da Tuya e chega na próxima fase. */}
        <span className="text-content flex-1 font-mono text-xs tabular-nums">{aparelho.ip}</span>

        <Etiqueta>v{aparelho.versao}</Etiqueta>
      </div>

      <p className="text-muted/70 mt-1.5 truncate pl-4 font-mono text-[10px]">{aparelho.id}</p>

      {aparelho.produto || !aparelho.ativo || !aparelho.suportado ? (
        <div className="mt-2 flex flex-wrap items-center gap-1.5 pl-4">
          {aparelho.produto ? <Etiqueta>{aparelho.produto}</Etiqueta> : null}
          {!aparelho.ativo ? <Etiqueta>sem pareamento</Etiqueta> : null}
          {!aparelho.suportado ? <Etiqueta alerta>protocolo não lido</Etiqueta> : null}
        </div>
      ) : null}

      {!aparelho.suportado ? (
        <p className="text-muted mt-2 pl-4 text-[10px] leading-relaxed">
          Fala o protocolo 3.5, que o Jarvis ainda não lê. Aparece aqui para você saber que ele
          existe, em vez de sumir e parecer problema de rede.
        </p>
      ) : null}
    </li>
  )
}

/** Um ponto, três significados. Cor é o canal, mas o `title` é o que garante a leitura. */
function Status({ ativo, suportado }: { ativo: boolean; suportado: boolean }) {
  const [cor, texto] = !suportado
    ? ['bg-muted/40', 'Encontrado, mas o protocolo dele ainda não é lido']
    : ativo
      ? ['bg-accent', 'Pareado e respondendo']
      : ['bg-muted/60', 'Ainda não pareado com nenhum app']

  return (
    <span
      title={texto}
      aria-label={texto}
      className={cn('h-2 w-2 shrink-0 rounded-full', cor, suportado && ativo && 'hud-glow')}
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
