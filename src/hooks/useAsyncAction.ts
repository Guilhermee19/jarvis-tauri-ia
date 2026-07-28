'use client'

import { useCallback, useState } from 'react'

/**
 * Ciclo "rodou / falhou" de um comando do backend.
 *
 * Existe porque as quatro seções do diagnóstico repetiriam o mesmo bloco de
 * `try/catch` com dois `useState`, e nenhuma delas tem nada a dizer sobre erro além
 * da mensagem que o Rust já mandou pronta.
 */
export function useAsyncAction() {
  const [isBusy, setIsBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const run = useCallback(async (action: () => Promise<void>) => {
    setIsBusy(true)
    setError(null)
    try {
      await action()
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      setIsBusy(false)
    }
  }, [])

  return { isBusy, error, run, setError }
}
