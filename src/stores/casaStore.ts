import { create } from 'zustand'
import { discoverDevices } from '@/lib/tauri'
import type { Aparelho } from '@/types'

/**
 * Os aparelhos da casa encontrados na rede.
 *
 * Mora numa store, e não no componente, porque a varredura leva segundos: fechar e
 * reabrir o painel no meio dela jogaria fora uma espera que já foi paga. O resultado
 * sobrevive ao painel; a busca, não é reiniciada por ele.
 *
 * Sem persistência em disco de propósito nesta fase. O que se descobre aqui é o estado
 * da rede AGORA — um aparelho que foi desligado da tomada não deve continuar na lista
 * porque estava lá ontem. Guardar em disco passa a fazer sentido na fase das chaves,
 * quando cada aparelho ganhar nome e segredo que valem entre sessões.
 */
interface CasaState {
  aparelhos: Aparelho[]
  procurando: boolean
  /** `null` antes da primeira busca — que é diferente de "buscou e não achou nada". */
  buscouEm: number | null
  /**
   * Pacotes que chegaram na rede sem virarem aparelho.
   *
   * Separa dois silêncios que dão a mesma tela vazia e têm soluções opostas: ninguém
   * falou (rede errada, firewall) e falaram num formato que não sabemos ler.
   */
  ignorados: number
  erro: string | null
  procurar: () => Promise<void>
}

function descrever(erro: unknown): string {
  return erro instanceof Error ? erro.message : String(erro)
}

export const useCasaStore = create<CasaState>((set, get) => ({
  aparelhos: [],
  procurando: false,
  buscouEm: null,
  ignorados: 0,
  erro: null,

  procurar: async () => {
    if (get().procurando) return
    set({ procurando: true, erro: null })

    try {
      const varredura = await discoverDevices()
      // Substitui a lista inteira em vez de mesclar: o que sumiu, sumiu — e ver um
      // aparelho velho na tela é pior que ver a lista encurtar.
      set({
        aparelhos: varredura.aparelhos,
        ignorados: varredura.ignorados,
        buscouEm: Date.now(),
      })
    } catch (erro) {
      set({ erro: descrever(erro) })
    } finally {
      set({ procurando: false })
    }
  },
}))
