'use client'

import { useEffect } from 'react'
import { useChatStore, useJanelaStore, useSettingsStore } from '@/stores'

/**
 * Carga inicial do app, uma vez só na raiz.
 *
 * O histórico é buscado aqui (e não dentro da tela de chat) porque a home também
 * mostra o resumo da conversa — trocar de aba não pode refazer a chamada.
 *
 * O arranjo das janelas entra pelo mesmo motivo, com um a mais: ele precisa acontecer
 * DEPOIS da montagem. O `localStorage` não existe na pré-renderização, e uma store que
 * nascesse com janelas abertas no navegador e fechadas no HTML gerado acusaria
 * divergência de hidratação.
 */
export function useBootstrap() {
  const loadSettings = useSettingsStore((state) => state.load)
  const loadHistory = useChatStore((state) => state.loadHistory)
  const hidratarJanelas = useJanelaStore((state) => state.hidratar)

  useEffect(() => {
    void loadSettings()
    void loadHistory()
    hidratarJanelas()
  }, [loadSettings, loadHistory, hidratarJanelas])
}
