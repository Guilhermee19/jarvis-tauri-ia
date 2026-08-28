'use client'

import { ChatPanel } from './ChatPanel'
import { FloatingPanel } from '@/components/ui/FloatingPanel'
import { useChatStore, useJanelaStore, zDaJanela } from '@/stores'

export function ChatWindow() {
  const abertas = useJanelaStore((state) => state.abertas)
  const abrir = useJanelaStore((state) => state.abrir)
  const fechar = useJanelaStore((state) => state.fechar)
  const isOpen = abertas.includes('chat')
  // O arranjo mora no `janelaStore`: o `FloatingPanel` some do DOM ao fechar, e agora
  // ele também precisa sobreviver ao fechamento do APP para a janela fixada reabrir onde
  // ficou. Maximizar continua sem mexer em posição nem tamanho — eles ficam guardados
  // intactos, e restaurar é só voltar a desenhá-los.
  const arranjo = useJanelaStore((state) => state.arranjos.chat)
  const ajustar = useJanelaStore((state) => state.ajustar)
  const fixadas = useJanelaStore((state) => state.fixadas)
  const fixar = useJanelaStore((state) => state.fixar)

  return (
    <FloatingPanel
      open={isOpen}
      onClose={() => fechar('chat')}
      zIndex={zDaJanela(abertas, 'chat')}
      onFocus={() => abrir('chat')}
      position={arranjo?.posicao ?? null}
      onPositionChange={(posicao) => ajustar('chat', { posicao })}
      size={arranjo?.tamanho ?? null}
      onSizeChange={(tamanho) => ajustar('chat', { tamanho })}
      maximized={arranjo?.maximizada ?? false}
      onMaximizedChange={(maximizada) => ajustar('chat', { maximizada })}
      fixada={fixadas.includes('chat')}
      onFixadaChange={(fixada) => fixar('chat', fixada)}
      title="Conversa"
      description="Converse com o assistente por texto."
      actions={<ClearHistoryAction />}
    >
      <ChatPanel />
    </FloatingPanel>
  )
}

function ClearHistoryAction() {
  const hasMessages = useChatStore((state) => state.messages.length > 0)
  const isTyping = useChatStore((state) => state.isTyping)
  const clear = useChatStore((state) => state.clear)

  if (!hasMessages) return null

  return (
    <button
      type="button"
      onClick={() => void clear()}
      disabled={isTyping}
      className="text-muted hover:text-danger shrink-0 text-[10px] tracking-[0.14em] uppercase transition-colors disabled:opacity-40"
    >
      Limpar
    </button>
  )
}
