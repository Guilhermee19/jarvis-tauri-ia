'use client'

import { useState } from 'react'

import { avaliarResposta } from '@/lib/tauri'
import { cn } from '@/lib/utils'
import type { ErroDaResposta, Veredito } from '@/types'

/**
 * As três pílulas, na ordem em que se pensa: acertou, quase, errou.
 *
 * Texto e não ícone, e isso é decisão e não preguiça: o `ui/icons.tsx` declara no topo que
 * o PRÓXIMO ícone a entrar ali é o gatilho para migrar tudo para uma biblioteca. Três
 * estados NOMEADOS também se leem melhor que três desenhos que cada um interpreta do seu
 * jeito — um joinha para baixo não diz se o erro foi de fato ou de jeito.
 */
const VEREDITOS: { id: Veredito; rotulo: string }[] = [
  { id: 'acertou', rotulo: 'Acertou' },
  { id: 'passou_perto', rotulo: 'Passou perto' },
  { id: 'errou', rotulo: 'Errou' },
]

/**
 * A pergunta que decide o que a correção vira.
 *
 * Errar o FATO se conserta com uma nota sobre aquele assunto, que volta quando o assunto
 * voltar. Responder MAL é sobre toda resposta e não tem assunto ao qual se prender: vira
 * regra no prompt. São guardados de formas diferentes porque funcionam de formas
 * diferentes, e só quem leu a resposta sabe qual dos dois foi.
 */
const TIPOS: { id: ErroDaResposta; rotulo: string; pergunta: string }[] = [
  { id: 'fato', rotulo: 'Errou o fato', pergunta: 'Qual era a resposta certa?' },
  { id: 'jeito', rotulo: 'Respondeu mal', pergunta: 'O que estava errado no jeito?' },
]

/** Pílula ligada/desligada, no mesmo molde dos filtros do painel de Conhecimento. */
function Pilula({
  rotulo,
  ligada,
  perigo,
  onClick,
}: {
  rotulo: string
  ligada: boolean
  perigo?: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={ligada}
      className={cn(
        'rounded-full border px-2 py-0.5 text-[10px] tracking-[0.1em] uppercase transition-colors',
        ligada
          ? perigo
            ? 'border-danger/40 bg-danger/10 text-danger'
            : 'border-accent/40 bg-accent/10 text-accent'
          : 'border-border-soft text-muted/60 hover:text-content',
      )}
    >
      {rotulo}
    </button>
  )
}

/**
 * O controle de avaliação de uma resposta do Jarvis.
 *
 * **"Acertou" é um clique e acabou.** Os outros dois abrem o campo da correção, porque é
 * ela que ensina: um "errou" sozinho não diz a um modelo de 3B o que era esperado, e o que
 * sobraria seria uma estatística que ninguém lê.
 *
 * Quem monta é o `MessageBubble`, e só para resposta do assistente que já terminou — o
 * porquê do "já terminou" está no `avaliarResposta`.
 */
export function AvaliacaoDaResposta({ id }: { id: string }) {
  const [veredito, setVeredito] = useState<Veredito | null>(null)
  const [tipo, setTipo] = useState<ErroDaResposta>('fato')
  const [rascunho, setRascunho] = useState('')
  const [salvo, setSalvo] = useState(false)
  const [erro, setErro] = useState<string | null>(null)

  async function mandar(escolhido: Veredito, comCorrecao?: string) {
    setErro(null)
    try {
      await avaliarResposta(id, escolhido, comCorrecao ? tipo : undefined, comCorrecao)
      setSalvo(true)
    } catch (falha) {
      setErro(falha instanceof Error ? falha.message : 'não consegui guardar')
    }
  }

  function escolher(escolhido: Veredito) {
    setVeredito(escolhido)
    setSalvo(false)
    setErro(null)

    // O elogio não tem o que perguntar: guarda na hora. Os outros dois esperam a correção,
    // que é a parte que vale.
    if (escolhido === 'acertou') {
      void mandar(escolhido)
    }
  }

  if (salvo) {
    return (
      <span className="text-muted/60 px-1 text-[10px]">
        {veredito === 'acertou' ? 'Anotado que acertou.' : 'Anotado. Ele já sabe.'}
      </span>
    )
  }

  const perguntaDoTipo = TIPOS.find((opcao) => opcao.id === tipo)?.pergunta

  return (
    <div className="flex flex-col gap-1.5 px-1">
      <div
        className="flex flex-wrap items-center gap-1.5"
        role="group"
        aria-label="Avaliar a resposta"
      >
        {VEREDITOS.map((opcao) => (
          <Pilula
            key={opcao.id}
            rotulo={opcao.rotulo}
            ligada={veredito === opcao.id}
            perigo={opcao.id === 'errou'}
            onClick={() => escolher(opcao.id)}
          />
        ))}
      </div>

      {veredito !== null && veredito !== 'acertou' && (
        <div className="border-border-soft flex flex-col gap-1.5 rounded-md border border-dashed p-2">
          <div className="flex flex-wrap gap-1.5">
            {TIPOS.map((opcao) => (
              <Pilula
                key={opcao.id}
                rotulo={opcao.rotulo}
                ligada={tipo === opcao.id}
                onClick={() => setTipo(opcao.id)}
              />
            ))}
          </div>

          <textarea
            value={rascunho}
            onChange={(evento) => setRascunho(evento.target.value)}
            placeholder={perguntaDoTipo}
            rows={2}
            className="border-border-soft bg-surface text-content placeholder:text-muted/60 focus-visible:ring-accent/40 w-full resize-none rounded-md border px-2 py-1.5 text-xs leading-relaxed outline-none focus-visible:ring-2"
          />

          <div className="flex items-center gap-2">
            <button
              type="button"
              disabled={rascunho.trim().length === 0}
              onClick={() => void mandar(veredito, rascunho.trim())}
              className="bg-surface-hover text-content hover:bg-border-soft rounded-md px-2 py-1 text-[10px] tracking-[0.1em] uppercase transition-colors disabled:opacity-50"
            >
              Ensinar
            </button>
            <span className="text-muted/60 text-[10px]">
              {tipo === 'fato' ? 'Vira nota sobre o assunto.' : 'Vira regra em toda resposta.'}
            </span>
          </div>
        </div>
      )}

      {erro !== null && <span className="text-danger text-[10px]">{erro}</span>}
    </div>
  )
}
