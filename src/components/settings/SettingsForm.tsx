'use client'

import { useState } from 'react'
import { Button } from '@/components/ui/Button'
import { Input } from '@/components/ui/Input'
import { DEFAULT_SETTINGS, type AppSettings } from '@/types'

interface SettingsFormProps {
  initial: AppSettings
  isSaving: boolean
  onSubmit: (settings: AppSettings) => void
  onCancel: () => void
}

/**
 * O form é separado do modal de propósito: o `Modal` desmonta o conteúdo ao fechar,
 * então o estado inicial vem das props a cada abertura — sem efeito de sincronização,
 * e sem rascunho de uma edição abandonada aparecendo depois.
 */
export function SettingsForm({ initial, isSaving, onSubmit, onCancel }: SettingsFormProps) {
  const [apiKey, setApiKey] = useState(initial.anthropicApiKey)
  const [assistantName, setAssistantName] = useState(initial.assistantName)
  const [elevenLabsKey, setElevenLabsKey] = useState(initial.elevenLabsApiKey)

  return (
    <div className="flex flex-col gap-4">
      <Input
        label="API key da Anthropic"
        type="password"
        value={apiKey}
        onChange={(event) => setApiKey(event.target.value)}
        placeholder="sk-ant-…"
        hint="Guardada em texto puro no arquivo de config do app. Ainda não é validada nem usada."
      />

      <Input
        label="API key da ElevenLabs"
        type="password"
        value={elevenLabsKey}
        onChange={(event) => setElevenLabsKey(event.target.value)}
        placeholder="sk_…"
        hint="Usada pelo TTS. Sem ela, o teste de voz no Diagnóstico fica inerte."
      />

      <Input
        label="Nome do assistente"
        value={assistantName}
        onChange={(event) => setAssistantName(event.target.value)}
        placeholder={DEFAULT_SETTINGS.assistantName}
        hint="Vai para o system prompt quando o agente real entrar."
      />

      <div className="flex justify-end gap-2 pt-1">
        <Button variant="ghost" onClick={onCancel} disabled={isSaving}>
          Cancelar
        </Button>
        <Button
          onClick={() =>
            onSubmit({
              // `initial` primeiro para não apagar o que o form não edita — a voz do
              // TTS é escolhida no Diagnóstico, não aqui.
              ...initial,
              anthropicApiKey: apiKey.trim(),
              elevenLabsApiKey: elevenLabsKey.trim(),
              assistantName: assistantName.trim() || DEFAULT_SETTINGS.assistantName,
            })
          }
          disabled={isSaving}
        >
          {isSaving ? 'Salvando…' : 'Salvar'}
        </Button>
      </div>
    </div>
  )
}
