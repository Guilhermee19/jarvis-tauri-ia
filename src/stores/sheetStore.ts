import { create } from 'zustand'

/**
 * Qual painel de feature está aberto.
 *
 * As features não são rotas: cada uma é um painel por cima do HUD, e só uma fica
 * aberta por vez — a barra de ícones é a barra de tarefas delas. A forma varia com
 * o uso: a conversa é uma janelinha flutuante, que o usuário arrasta para onde
 * quiser; as configurações são uma gaveta na borda direita, que se lê e se fecha.
 * Adicionar uma feature nova (voz, automação) é adicionar um id aqui.
 */
export type SheetId = 'chat' | 'settings'

interface SheetState {
  activeSheet: SheetId | null
  open: (id: SheetId) => void
  close: () => void
  /** Semântica de barra de tarefas: clicar no ícone da gaveta aberta a fecha. */
  toggle: (id: SheetId) => void
}

export const useSheetStore = create<SheetState>((set) => ({
  activeSheet: null,
  open: (id) => set({ activeSheet: id }),
  close: () => set({ activeSheet: null }),
  toggle: (id) => set((state) => ({ activeSheet: state.activeSheet === id ? null : id })),
}))
