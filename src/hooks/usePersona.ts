'use client'

import { useEffect } from 'react'
import { useSettingsStore } from '@/stores'

/**
 * Escreve o tema escolhido no `<html>`, e é isso que retinge o app.
 *
 * Um atributo, e não classes espalhadas pelos componentes: o `globals.css` redefine os
 * tokens de cor em `:root[data-persona='ultron']`, e como TODA cor do app sai desses
 * tokens (`bg-surface`, `text-accent`, e o `color-mix` da grade e da vinheta), trocar o
 * atributo repinta a interface inteira sem nenhum componente saber que o tema existe.
 *
 * **Não precisa reiniciar o app.** A cor é CSS, o nome e o gatilho de voz vêm do store, e
 * o system prompt é montado a cada mensagem — não há nada congelado no boot para
 * invalidar. Reiniciar só custaria o estado das janelinhas e alguns segundos.
 */
export function usePersona() {
  const persona = useSettingsStore((state) => state.settings.persona)

  useEffect(() => {
    document.documentElement.dataset.persona = persona
  }, [persona])
}
