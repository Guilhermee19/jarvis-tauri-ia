'use client'

import { useEffect } from 'react'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { JarvisEvent, onJarvisEvent } from '@/lib/tauri'
import { useSensorStore } from '@/stores'

/**
 * Assina o nível do microfone uma vez só, no shell da janela.
 *
 * Fica aqui e não em quem desenha o medidor porque hoje são dois (o botão da barra
 * e a bancada de diagnóstico): cada um assinando por conta própria criaria
 * listeners duplicados para o mesmo evento.
 */
export function useSensorEvents() {
  const setMicLevel = useSensorStore((state) => state.setMicLevel)

  useEffect(() => {
    let unlisten: UnlistenFn | undefined
    let cancelled = false

    void onJarvisEvent<number>(JarvisEvent.MicLevel, setMicLevel).then((fn) => {
      // O `listen` é assíncrono: se desmontar antes, cancela na hora.
      if (cancelled) fn()
      else unlisten = fn
    })

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [setMicLevel])
}
