'use client'

import { useEffect, useRef } from 'react'
import { identifyTrack, nowPlaying } from '@/lib/tauri'
import { faixaDoTitulo, useNowPlayingStore } from '@/stores'

/** Passo do relógio local. Um segundo é a menor unidade que a barra mostra. */
const TIQUE_MS = 1000

/**
 * A cada quantos tiques perguntar ao Spotify o que está acontecendo.
 *
 * Enumerar janelas custa pouco, mas não é grátis: a cada 3 s pega pausa e troca de
 * faixa rápido o bastante para ninguém notar, e mantém o app quieto no resto do tempo.
 */
const TIQUES_POR_SINCRONIA = 3

/**
 * Mantém o widget de música batendo com a realidade.
 *
 * Roda SEMPRE, não só depois que o Jarvis toca algo: é assim que o widget aparece
 * quando você já estava ouvindo música antes de abrir o app, ou quando trocou de
 * faixa pelo próprio Spotify.
 *
 * A busca da capa só acontece quando o TÍTULO MUDA. Perguntar ao Spotify a cada
 * sincronia gastaria a cota da API para receber sempre a mesma resposta.
 */
export function useNowPlaying() {
  const avancar = useNowPlayingStore((state) => state.avancar)
  const definir = useNowPlayingStore((state) => state.definir)
  const pausar = useNowPlayingStore((state) => state.pausar)
  const retomar = useNowPlayingStore((state) => state.retomar)

  /** Guarda contra duas identificações simultâneas do mesmo título. */
  const identificando = useRef<string | null>(null)

  useEffect(() => {
    let tiques = 0
    let vivo = true

    async function sincronizar() {
      const { titulo, tocando } = await nowPlaying()
      if (!vivo) return

      if (!tocando || !titulo) {
        pausar()
        return
      }

      const { precisaIdentificar, faixa } = useNowPlayingStore.getState()
      if (!precisaIdentificar(titulo)) {
        retomar()
        return
      }

      if (identificando.current === titulo) return
      identificando.current = titulo

      // Se JÁ havia uma faixa, vimos a troca acontecer e o zero é verdade. Se não
      // havia, o app abriu no meio da música e não dá para saber onde ela está.
      const confiavel = faixa !== null

      const achada = await identifyTrack(titulo).catch(() => null)
      if (!vivo) return

      // Sem credencial ou sem resultado, o texto do título ainda vale mais que nada.
      definir(achada ?? faixaDoTitulo(titulo), confiavel)
      identificando.current = null
    }

    const relogio = setInterval(() => {
      avancar(TIQUE_MS)

      tiques += 1
      if (tiques % TIQUES_POR_SINCRONIA !== 0) return

      // Fora do runtime do Tauri isto sempre falha; o widget segue como estiver.
      void sincronizar().catch(() => {})
    }, TIQUE_MS)

    // Primeira leitura na hora: quem abre o app com música tocando não deve esperar
    // três segundos pelo widget.
    void sincronizar().catch(() => {})

    return () => {
      vivo = false
      clearInterval(relogio)
    }
  }, [avancar, definir, pausar, retomar])
}
