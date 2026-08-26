import { create } from 'zustand'
import type { Faixa } from '@/lib/tauri'

export interface WidgetPosition {
  x: number
  y: number
}

/**
 * O que o widget de "tocando agora" mostra.
 *
 * **O tempo decorrido é contado AQUI, não lido do Spotify.** Ler a posição real exige
 * `GET /v1/me/player`, que precisa do fluxo completo de OAuth com consentimento no
 * navegador — e o app usa só *client credentials*, que não alcançam o player do
 * usuário. O que dá para saber sem isso é o título da janela, e ele responde a
 * pergunta que importa: está tocando ou pausado.
 *
 * Daí o [`posicaoConfiavel`]: se vimos a faixa COMEÇAR, contar do zero está certo. Se
 * o app abriu com a música no meio, não sabemos onde ela está — e aí a barra some em
 * vez de mostrar um número inventado.
 *
 * ponytail: contador local. Vira posição real quando (e se) o OAuth entrar.
 */
interface NowPlayingState {
  faixa: Faixa | null
  tocando: boolean
  decorridoMs: number
  /** `false` quando o app achou a música já tocando: não dá para saber a posição. */
  posicaoConfiavel: boolean
  /** `null` enquanto ninguém arrastou: o widget nasce no canto de baixo à direita. */
  posicao: WidgetPosition | null
  /**
   * Título que o usuário fechou no ✕. Sem isto, o laço reabriria o widget no segundo
   * seguinte e o botão de fechar não fecharia nada.
   */
  dispensado: string | null

  /** O Jarvis acabou de mandar tocar: sabemos que começou agora. */
  mostrar: (faixa: Faixa) => void
  /** Achamos tocando pelo título da janela. */
  definir: (faixa: Faixa, confiavel: boolean) => void
  pausar: () => void
  retomar: () => void
  fechar: () => void
  mover: (posicao: WidgetPosition) => void
  /** Um passo do relógio local. Só anda com a música tocando. */
  avancar: (ms: number) => void
  /** `true` quando o título que está tocando não é o que o widget mostra. */
  precisaIdentificar: (titulo: string) => boolean
}

/**
 * Compara ignorando acento, caixa e pontuação — os dois lados escrevem diferente.
 *
 * `includes` nos dois sentidos porque o Spotify às vezes acrescenta " - Ao Vivo" no
 * título da janela, ou corta o nome do artista quando fica longo demais.
 */
export function mesmaFaixa(faixa: Faixa, titulo: string): boolean {
  const limpar = (texto: string) =>
    texto
      .normalize('NFD')
      .replace(/\p{Diacritic}/gu, '')
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, ' ')
      .trim()

  const nosso = limpar(`${faixa.artista} ${faixa.titulo}`)
  const deles = limpar(titulo)
  if (!nosso || !deles) return false

  return nosso.includes(deles) || deles.includes(nosso)
}

/** Último recurso: o Spotify não achou a faixa, então só temos o texto do título. */
export function faixaDoTitulo(titulo: string): Faixa {
  const [artista, ...resto] = titulo.split(' - ')
  return {
    id: '',
    titulo: resto.join(' - ') || titulo,
    artista: resto.length > 0 ? artista : '',
    capa: null,
    duracaoMs: 0,
  }
}

export const useNowPlayingStore = create<NowPlayingState>((set, get) => ({
  faixa: null,
  tocando: false,
  decorridoMs: 0,
  posicaoConfiavel: false,
  posicao: null,
  dispensado: null,

  mostrar: (faixa) =>
    set({
      faixa,
      tocando: true,
      decorridoMs: 0,
      posicaoConfiavel: true,
      // Pedir a música de novo é motivo de sobra para o widget voltar.
      dispensado: null,
    }),

  definir: (faixa, confiavel) =>
    set({ faixa, tocando: true, decorridoMs: 0, posicaoConfiavel: confiavel }),

  pausar: () => set({ tocando: false }),
  retomar: () => set({ tocando: true }),

  fechar: () => {
    const { faixa } = get()
    set({
      faixa: null,
      tocando: false,
      decorridoMs: 0,
      dispensado: faixa ? `${faixa.artista} - ${faixa.titulo}` : null,
    })
  },

  mover: (posicao) => set({ posicao }),

  avancar: (ms) => {
    const { tocando, faixa, decorridoMs } = get()
    if (!tocando || !faixa) return

    // Sem duração conhecida o contador sobe livre; com duração, ele para no fim em
    // vez de passar do total.
    const proximo = decorridoMs + ms
    set({
      decorridoMs: faixa.duracaoMs > 0 ? Math.min(proximo, faixa.duracaoMs) : proximo,
    })
  },

  precisaIdentificar: (titulo) => {
    const { faixa, dispensado } = get()
    if (dispensado && mesmaFaixa(faixaDoTitulo(dispensado), titulo)) return false
    return faixa === null || !mesmaFaixa(faixa, titulo)
  },
}))
