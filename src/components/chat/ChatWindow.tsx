'use client'

import { useState } from 'react'
import { ChatPanel } from './ChatPanel'
import { FloatingPanel, type PanelPosition, type PanelSize } from '@/components/ui/FloatingPanel'
import { useChatStore, useSheetStore } from '@/stores'

export function ChatWindow() {
  const isOpen = useSheetStore((state) => state.activeSheet === 'chat')
  const close = useSheetStore((state) => state.close)
  // Posição e tamanho moram aqui, e não no `FloatingPanel`: ele some do DOM ao fechar,
  // e a janelinha precisa reabrir onde e do jeito que o usuário a deixou.
  const [position, setPosition] = useState<PanelPosition | null>(null)
  const [size, setSize] = useState<PanelSize | null>(null)
  // Maximizar não mexe em `position` nem em `size`: eles ficam guardados intactos, e
  // restaurar é só voltar a desenhá-los. Sem estado extra para reconciliar.
  const [maximized, setMaximized] = useState(false)

  return (
    <FloatingPanel
      open={isOpen}
      onClose={close}
      position={position}
      onPositionChange={setPosition}
      size={size}
      onSizeChange={setSize}
      maximized={maximized}
      onMaximizedChange={setMaximized}
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
