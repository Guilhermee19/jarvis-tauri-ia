'use client'

import { useEffect } from 'react'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { JarvisEvent, onJarvisEvent } from '@/lib/tauri'
import { useJanelaStore } from '@/stores'

/** Escuta os eventos que a bandeja do sistema dispara (hoje: abrir Configurações). */
export function useTrayEvents() {
  const abrirGaveta = useJanelaStore((state) => state.abrirGaveta)

  useEffect(() => {
    let unlisten: UnlistenFn | undefined
    let cancelled = false

    void onJarvisEvent(JarvisEvent.OpenSettings, () => abrirGaveta('settings')).then((fn) => {
      // O `listen` é assíncrono: se o componente desmontar antes, cancela na hora.
      if (cancelled) fn()
      else unlisten = fn
    })

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [abrirGaveta])
}
