'use client'

import { useChatStore } from '@/stores/chatStore'
import { cn, formatTime } from '@/lib/utils'
import type { ChatMessage } from '@/types'

import { AvaliacaoDaResposta } from './AvaliacaoDaResposta'

/**
 * A partir de quantos caracteres a mensagem deixa de ser bolha.
 *
 * **É o número que resolve a reclamação de "texto grande fica feio".** Bolha é boa para
 * troca curta — "pausa a música" / "Pausado." lê como conversa. Mas uma resposta de
 * quatro parágrafos dentro de uma bolha de 85% de uma coluna de 560px vira uma tira
 * estreita e altíssima, e é isso que dá a impressão de rolagem infinita: o texto não
 * está com scroll a mais, está com largura de menos.
 *
 * É a solução do Discord, e é por isso que texto longo lê bem lá: mensagem comprida ocupa
 * a largura toda, com o nome de quem falou em cima em vez de um lado.
 *
 * 280 é onde a bolha passa de ~4 linhas na largura da coluna. Abaixo disso a bolha ainda
 * ganha; acima, ela só atrapalha.
 */
const LIMITE_DA_BOLHA = 280

interface MessageBubbleProps {
  message: ChatMessage
  assistantName: string
}

/** Bolhas por papel. `system` é o log de gatilho e ação que o agente empurra. */
export function MessageBubble({ message, assistantName }: MessageBubbleProps) {
  const isUser = message.role === 'user'

  /*
   * **Só depois que ele terminou de falar**, e a razão é um `id` que troca no meio.
   *
   * Enquanto a resposta chega em fluxo, a bolha vive com um UUID gerado AQUI no frontend
   * (`chatStore.receberFrase`), que o `loadHistory` do fim do turno joga fora quando as
   * mensagens voltam do Rust com o id de verdade. Uma avaliação presa àquele id nasceria
   * apontando para uma mensagem que deixou de existir segundos depois.
   */
  const emCurso = useChatStore((state) => state.respostaEmCurso)
  const podeAvaliar = !isUser && message.role === 'assistant' && message.id !== emCurso

  // Sem bolha e sem lado: isto é registro da máquina, não fala. A borda tracejada é
  // o que separa as duas coisas sem precisar inventar uma cor nova.
  if (message.role === 'system') {
    return (
      <div className="border-border-soft bg-surface/40 rounded-md border border-dashed px-3 py-2">
        <div className="text-accent pb-1 text-[9px] tracking-[0.22em] uppercase">
          Log · {formatTime(message.timestamp)}
        </div>
        {/* Minúsculo e monoespaçado de propósito: alinha as colunas do trace e deixa
            o olho pular por cima quando não interessa.

            `overflow-x-auto` porque o trace tem linhas longas que NÃO devem quebrar — as
            colunas dele só alinham se cada linha ficar inteira. É o mesmo tratamento que
            o Discord dá a bloco de código: o texto corre, a página não. */}
        <pre className="scroll-thin text-muted overflow-x-auto font-mono text-[10px] leading-relaxed whitespace-pre">
          {message.content}
        </pre>
      </div>
    )
  }

  if (message.content.length > LIMITE_DA_BOLHA) {
    return (
      <Corrida
        message={message}
        assistantName={assistantName}
        isUser={isUser}
        podeAvaliar={podeAvaliar}
      />
    )
  }

  return (
    <div className={cn('flex w-full', isUser ? 'justify-end' : 'justify-start')}>
      {/* `min-w-0` no filho do flex: sem ele, uma palavra sem espaço (uma URL, um hash)
          impede o encolhimento e a bolha estoura a coluna, criando rolagem HORIZONTAL na
          lista inteira. É o defeito que mais parecia "scroll feio". */}
      <div
        className={cn(
          'flex max-w-[85%] min-w-0 flex-col gap-1',
          isUser ? 'items-end' : 'items-start',
        )}
      >
        <div
          className={cn(
            // `break-words` é o par do `min-w-0`: um segura o flex, o outro quebra a
            // palavra. Sem os dois juntos, uma URL longa continua vazando.
            'rounded-2xl px-3.5 py-2 text-sm leading-relaxed break-words whitespace-pre-wrap',
            isUser
              ? 'bg-accent-strong rounded-br-md text-white'
              : 'border-border-soft bg-surface text-content rounded-bl-md border',
          )}
        >
          {message.content}
        </div>
        <span className="text-muted px-1 text-[10px]">{formatTime(message.timestamp)}</span>
        {podeAvaliar && <AvaliacaoDaResposta id={message.id} />}
      </div>
    </div>
  )
}

/**
 * Mensagem longa: largura cheia, com cabeçalho em cima em vez de bolha ao lado.
 *
 * A cor do NOME é o que substitui o lado e a bolha para dizer quem falou — dois sinais
 * viraram um, e é o suficiente porque as mensagens se alternam. A barra à esquerda no
 * lado do usuário existe para o olho achar o começo do próprio texto ao rolar para trás,
 * que é o que a borda da bolha fazia antes.
 */
function Corrida({
  message,
  assistantName,
  isUser,
  podeAvaliar,
}: MessageBubbleProps & { isUser: boolean; podeAvaliar: boolean }) {
  return (
    <div
      className={cn(
        'flex min-w-0 flex-col gap-1 py-0.5',
        isUser && 'border-accent-strong/50 border-l-2 pl-3',
      )}
    >
      <div className="flex items-baseline gap-2">
        <span
          className={cn(
            'text-[11px] font-semibold tracking-wide',
            isUser ? 'text-accent' : 'text-content',
          )}
        >
          {isUser ? 'Você' : assistantName}
        </span>
        <span className="text-muted text-[10px]">{formatTime(message.timestamp)}</span>
      </div>

      {/* `leading-relaxed` continua, mas agora numa linha de ~70 caracteres em vez de
          ~40: é a largura que faz o texto longo ser lido em vez de escaneado. */}
      <div className="text-content text-sm leading-relaxed break-words whitespace-pre-wrap">
        {message.content}
      </div>

      {podeAvaliar && <AvaliacaoDaResposta id={message.id} />}
    </div>
  )
}
