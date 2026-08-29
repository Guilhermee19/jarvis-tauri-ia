'use client'

import { useEffect } from 'react'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { JarvisEvent, onJarvisEvent, type MudouDeEndereco, type UiAction } from '@/lib/tauri'
import { useNavegadorStore, useNowPlayingStore, useSensorStore } from '@/stores'

/**
 * Assina os eventos de sensor uma vez só, no shell da janela.
 *
 * Fica aqui e não em quem desenha o medidor porque hoje são dois (o botão da barra
 * e a bancada de diagnóstico): cada um assinando por conta própria criaria
 * listeners duplicados para o mesmo evento.
 */
export function useSensorEvents() {
  const setMicLevel = useSensorStore((state) => state.setMicLevel)
  const setTtsLevel = useSensorStore((state) => state.setTtsLevel)
  const setWebcam = useSensorStore((state) => state.setWebcam)
  const mostrarFaixa = useNowPlayingStore((state) => state.mostrar)
  const abrirSite = useNavegadorStore((state) => state.abrirSite)
  const anotarEndereco = useNavegadorStore((state) => state.anotarEndereco)
  const pesquisar = useNavegadorStore((state) => state.pesquisar)

  useEffect(() => {
    const pendentes: UnlistenFn[] = []
    let cancelled = false

    function assinar(promessa: Promise<UnlistenFn>) {
      void promessa.then((fn) => {
        // O `listen` é assíncrono: se desmontar antes, cancela na hora.
        if (cancelled) fn()
        else pendentes.push(fn)
      })
    }

    assinar(onJarvisEvent<number>(JarvisEvent.MicLevel, setMicLevel))
    assinar(onJarvisEvent<number>(JarvisEvent.TtsLevel, setTtsLevel))

    // O navegador se mexendo por conta própria. Os dois vêm de callbacks do webview, que
    // rodam na thread principal do Tauri: por isso eles AVISAM em vez de agir — criar uma
    // aba lá dentro trava o app. Quem age é a store, daqui, por comando `async`.
    assinar(
      onJarvisEvent<MudouDeEndereco>(JarvisEvent.BrowserUrl, (mudou) =>
        anotarEndereco(mudou.id, mudou.url),
      ),
    )
    assinar(
      onJarvisEvent<string>(JarvisEvent.BrowserNewTab, (url) => void abrirSite(url)),
    )

    // O agente pedindo à UI. "abre a webcam" cai no MESMO caminho do botão da barra
    // de ícones — é isso que mantém o botão aceso e o preview rodando.
    assinar(
      onJarvisEvent<UiAction>(JarvisEvent.UiAction, (acao) => {
        switch (acao.tipo) {
          case 'webcam-on':
            void setWebcam(true)
            break
          case 'webcam-off':
            void setWebcam(false)
            break
          case 'tocando':
            mostrarFaixa(acao.faixa)
            break
          // "abre o youtube" abre uma aba AQUI DENTRO, e pelo mesmo caminho da barra de
          // endereço: é a store que abre a janelinha antes de criar o webview, e sem essa
          // ordem a aba nasceria sem um buraco onde caber.
          case 'abrir-site':
            void abrirSite(acao.url)
            break
          case 'pesquisar':
            void pesquisar(acao.query)
            break
        }
      }),
    )

    return () => {
      cancelled = true
      pendentes.forEach((fn) => fn())
    }
  }, [setMicLevel, setTtsLevel, setWebcam, mostrarFaixa, abrirSite, pesquisar, anotarEndereco])
}
