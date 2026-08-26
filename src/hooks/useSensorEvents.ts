'use client'

import { useEffect } from 'react'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { JarvisEvent, onJarvisEvent, type UiAction } from '@/lib/tauri'
import { useSensorStore } from '@/stores'

/**
 * Assina os eventos de sensor uma vez só, no shell da janela.
 *
 * Fica aqui e não em quem desenha o medidor porque hoje são dois (o botão da barra
 * e a bancada de diagnóstico): cada um assinando por conta própria criaria
 * listeners duplicados para o mesmo evento.
 */
export function useSensorEvents() {
  const setMicLevel = useSensorStore((state) => state.setMicLevel)
  const setWebcam = useSensorStore((state) => state.setWebcam)

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

    // "abre a webcam" chega por aqui e cai no MESMO caminho do botão da barra de
    // ícones — é isso que mantém o botão aceso e o preview rodando.
    assinar(
      onJarvisEvent<UiAction>(JarvisEvent.UiAction, (acao) => {
        if (acao === 'webcam-on') void setWebcam(true)
        if (acao === 'webcam-off') void setWebcam(false)
      }),
    )

    return () => {
      cancelled = true
      pendentes.forEach((fn) => fn())
    }
  }, [setMicLevel, setWebcam])
}
