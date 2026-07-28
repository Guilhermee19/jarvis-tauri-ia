'use client'

import { useEffect } from 'react'
import { useChatStore, useSettingsStore } from '@/stores'

/**
 * Carga inicial do app, uma vez só na raiz.
 *
 * O histórico é buscado aqui (e não dentro da tela de chat) porque a home também
 * mostra o resumo da conversa — trocar de aba não pode refazer a chamada.
 */
export function useBootstrap() {
  const loadSettings = useSettingsStore((state) => state.load)
  const loadHistory = useChatStore((state) => state.loadHistory)

  useEffect(() => {
    void loadSettings()
    void loadHistory()
  }, [loadSettings, loadHistory])
}
