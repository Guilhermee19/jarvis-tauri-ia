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
  const [ollamaUrl, setOllamaUrl] = useState(initial.ollamaUrl)
  const [ollamaModel, setOllamaModel] = useState(initial.ollamaModel)
  const [memoriaPath, setMemoriaPath] = useState(initial.memoriaPath)
  const [braveKey, setBraveKey] = useState(initial.braveApiKey)
  const [spotifyId, setSpotifyId] = useState(initial.spotifyClientId)
  const [spotifySecret, setSpotifySecret] = useState(initial.spotifyClientSecret)

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
        label="Servidor do Ollama"
        value={ollamaUrl}
        onChange={(event) => setOllamaUrl(event.target.value)}
        placeholder={DEFAULT_SETTINGS.ollamaUrl}
        hint="Onde roda o intérprete de comandos. Precisa do Ollama instalado e ativo."
      />

      <Input
        label="Modelo do intérprete"
        value={ollamaModel}
        onChange={(event) => setOllamaModel(event.target.value)}
        placeholder={DEFAULT_SETTINGS.ollamaModel}
        hint="Baixe com `ollama pull qwen2.5vl:3b`. Precisa ser multimodal para o `o que é isso?` funcionar. Vazio desliga o intérprete."
      />

      <Input
        label="API key do Brave Search"
        type="password"
        value={braveKey}
        onChange={(event) => setBraveKey(event.target.value)}
        placeholder="BSA…"
        hint="Sem ela a busca usa a Wikipedia, que responde 'quem foi X' mas não sabe preço nem notícia. Grátis em brave.com/search/api (2000 buscas/mês)."
      />

      <Input
        label="Spotify — Client ID"
        value={spotifyId}
        onChange={(event) => setSpotifyId(event.target.value)}
        placeholder="sem isso, 'toque X' só abre a busca"
        hint="Crie um app em developer.spotify.com/dashboard (grátis, 2 min, sem login de usuário). É o que permite achar a faixa exata e tocar direto."
      />

      <Input
        label="Spotify — Client Secret"
        type="password"
        value={spotifySecret}
        onChange={(event) => setSpotifySecret(event.target.value)}
        placeholder="…"
      />

      <Input
        label="Pasta da memória"
        value={memoriaPath}
        onChange={(event) => setMemoriaPath(event.target.value)}
        placeholder="memoria/ do projeto"
        hint="Onde ele guarda o que aprende, em markdown. Dá para abrir no Obsidian."
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
              ollamaUrl: ollamaUrl.trim() || DEFAULT_SETTINGS.ollamaUrl,
              // Sem fallback aqui de propósito: vazio é uma escolha válida (desliga
              // o intérprete), diferente do nome e da URL.
              ollamaModel: ollamaModel.trim(),
              // Vazio também é válido: cai na pasta padrão.
              memoriaPath: memoriaPath.trim(),
              braveApiKey: braveKey.trim(),
              spotifyClientId: spotifyId.trim(),
              spotifyClientSecret: spotifySecret.trim(),
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
