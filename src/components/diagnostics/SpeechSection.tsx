'use client'

import { useState } from 'react'
import { Section } from './Section'
import { Button } from '@/components/ui/Button'
import { useAsyncAction } from '@/hooks/useAsyncAction'
import { listVoices, speakText } from '@/lib/tauri'
import { useSettingsStore } from '@/stores'
import type { Voice } from '@/types'

/** Frase fixa do teste: curta, em português, e com número para checar a prosódia. */
const TEST_PHRASE = 'Sistemas online. Sou o Jarvis, e estou ouvindo você.'

export function SpeechSection() {
  const settings = useSettingsStore((state) => state.settings)
  const save = useSettingsStore((state) => state.save)
  const [voices, setVoices] = useState<Voice[]>([])
  const { isBusy, error, run } = useAsyncAction()

  const hasKey = settings.elevenLabsApiKey.length > 0

  return (
    <Section
      title="Voz"
      hint="Sintetiza uma frase fixa na ElevenLabs e toca no alto-falante padrão. Precisa da key da ElevenLabs em Configurações."
      error={error}
    >
      <div className="flex flex-wrap items-center gap-2">
        <Button onClick={() => void run(() => speakText(TEST_PHRASE))} disabled={isBusy || !hasKey}>
          Testar voz
        </Button>
        <Button
          variant="subtle"
          onClick={() => void run(async () => setVoices(await listVoices()))}
          disabled={isBusy || !hasKey}
        >
          Carregar vozes
        </Button>
      </div>

      {!hasKey ? (
        <p className="text-muted text-[11px]">
          Sem key da ElevenLabs: os dois botões ficam inertes.
        </p>
      ) : null}

      {voices.length > 0 ? (
        <label className="flex flex-col gap-1">
          <span className="text-muted text-[10px] tracking-[0.14em] uppercase">Voz</span>
          {/* Escolher já salva nas configurações: é a mesma preferência que o agente
              da v0.2 vai ler quando falar sem passar `voiceId`. */}
          <select
            value={settings.ttsVoiceId}
            onChange={(event) => void save({ ...settings, ttsVoiceId: event.target.value })}
            className="border-border-soft bg-base text-content focus:border-accent rounded-lg border px-2 py-1.5 text-sm focus:outline-none"
          >
            <option value="">Primeira voz da conta</option>
            {voices.map((voice) => (
              <option key={voice.id} value={voice.id}>
                {voice.description ? `${voice.name} — ${voice.description}` : voice.name}
              </option>
            ))}
          </select>
        </label>
      ) : null}
    </Section>
  )
}
