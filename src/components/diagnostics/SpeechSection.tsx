'use client'

import { useState } from 'react'
import { Section } from './Section'
import { Button } from '@/components/ui/Button'
import { useAsyncAction } from '@/hooks/useAsyncAction'
import { listVoices, speakText } from '@/lib/tauri'
import { useSettingsStore } from '@/stores'
import { NOME_DA_PERSONA, type Voice } from '@/types'

/** Frase fixa do teste: curta, em português, e com número para checar a prosódia. */
const TEST_PHRASE = 'Sistemas online. Sou o Jarvis, e estou ouvindo você.'

export function SpeechSection() {
  const settings = useSettingsStore((state) => state.settings)
  const save = useSettingsStore((state) => state.save)
  const [voices, setVoices] = useState<Voice[]>([])
  const { isBusy, error, run } = useAsyncAction()

  const hasKey = settings.elevenLabsApiKey.length > 0

  // Cada tema guarda a sua voz num campo próprio; este editor mexe no do tema ativo.
  const campoDaVoz = settings.persona === 'ultron' ? 'ttsVoiceUltron' : 'ttsVoiceJarvis'
  const vozAtual = settings[campoDaVoz]

  return (
    <Section
      title="Voz"
      hint="Sintetiza uma frase fixa na ElevenLabs e toca no alto-falante padrão. A key precisa da permissão text_to_speech; listar o catálogo pede voices_read a mais — sem ela, cole o ID da voz no campo abaixo."
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

      {hasKey ? (
        <label className="flex flex-col gap-1">
          {/* A voz é UMA POR TEMA: este campo edita a do tema ativo. Trocar de tema em
              Configurações traz o outro campo para cá, e cada um guarda a sua — sem
              isso, virar Ultron manteria a voz do Jarvis e a troca ficaria pela metade. */}
          <span className="text-muted text-[10px] tracking-[0.14em] uppercase">
            Voz do {NOME_DA_PERSONA[settings.persona]}
          </span>
          {/* Campo de texto com `datalist`, e não um `<select>`: o catálogo exige a
              permissão `voices_read`, que uma key restrita pode não ter — e aí um
              select vazio deixava o app num beco sem saída, porque com `ttsVoiceId`
              vazio o backend cai justamente em listar o catálogo para achar a
              primeira voz. Colar o ID (o botão "Copy voice ID" do site da ElevenLabs)
              pula o catálogo inteiro: falar só pede `text_to_speech`.

              O `datalist` é o mesmo controle servindo às duas rotas — quem carregou
              as vozes escolhe pelo nome na lista, quem não pode listar digita. */}
          <input
            type="text"
            list="tts-voices"
            value={vozAtual}
            onChange={(event) => void save({ ...settings, [campoDaVoz]: event.target.value })}
            placeholder="ID da voz — vazio usa a primeira da conta (pede voices_read)"
            spellCheck={false}
            className="border-border-soft bg-base text-content placeholder:text-muted/60 focus:border-accent rounded-lg border px-2 py-1.5 text-sm focus:outline-none"
          />
          <datalist id="tts-voices">
            {voices.map((voice) => (
              <option key={voice.id} value={voice.id}>
                {voice.description ? `${voice.name} — ${voice.description}` : voice.name}
              </option>
            ))}
          </datalist>
        </label>
      ) : null}
    </Section>
  )
}
