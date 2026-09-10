'use client'

import { useEffect } from 'react'
import { useChatStore, useJanelaStore, useSensorStore, useSettingsStore } from '@/stores'

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
 *
 * ## A escuta abre junto com o app
 *
 * Um assistente de voz que precisa ser ligado no botão antes de ouvir é um assistente
 * que não está ouvindo — quem chega perto da máquina e fala o nome dele espera resposta,
 * não descobrir que faltava um clique. Por isso ela sobe aqui, e não no clique do
 * microfone: o botão da barra continua existindo para CALAR.
 *
 * `setListening(true)` e não `toggleListening()` porque o efeito roda duas vezes no modo
 * estrito do `dev`, e um toggle em dobro desligaria o que o primeiro ligou.
 *
 * Falhar aqui não trava nada: sem microfone (arrancado, sem permissão) o motivo fica em
 * `dictationError`, que a barra já desenha, e o resto do app abre igual.
 */
export function useBootstrap() {
  const loadSettings = useSettingsStore((state) => state.load)
  const loadHistory = useChatStore((state) => state.loadHistory)
  const hidratarJanelas = useJanelaStore((state) => state.hidratar)
  const setListening = useSensorStore((state) => state.setListening)

  useEffect(() => {
    void loadSettings()
    void loadHistory()
    hidratarJanelas()
    void setListening(true)
  }, [loadSettings, loadHistory, hidratarJanelas, setListening])
}
