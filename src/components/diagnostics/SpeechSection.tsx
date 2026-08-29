'use client'

import { useState } from 'react'
import { Section } from './Section'
import { Button } from '@/components/ui/Button'
import { useAsyncAction } from '@/hooks/useAsyncAction'
import { escolherClipeDeVoz, listVoices, speakText, uploadVoiceReference } from '@/lib/tauri'
import { useSettingsStore } from '@/stores'
import { NOME_DA_PERSONA, vozDaPersona, type Voice } from '@/types'

/** Frase fixa do teste: curta, em português, e com número para checar a prosódia. */
const TEST_PHRASE = 'Sistemas online. Sou o Jarvis, e estou ouvindo você.'

export function SpeechSection() {
  const settings = useSettingsStore((state) => state.settings)
  const save = useSettingsStore((state) => state.save)
  const [voices, setVoices] = useState<Voice[]>([])
  const { isBusy, error, run } = useAsyncAction()

  // Cada tema guarda o seu clipe num campo próprio; este editor mexe no do tema ativo.
  const campoDaVoz = settings.persona === 'ultron' ? 'ttsVoiceUltron' : 'ttsVoiceJarvis'
  const vozAtual = vozDaPersona(settings)
  const temVoz = vozAtual.trim().length > 0

  /** Escolhe um arquivo, manda para o servidor, e guarda o nome que ele devolveu. */
  const cadastrar = () =>
    void run(async () => {
      const caminho = await escolherClipeDeVoz()
      if (!caminho) return

      // O nome vem do servidor, e não do caminho escolhido: ele higieniza o nome do
      // arquivo, e guardar o nosso daria uma voz que não existe do lado de lá.
      const nome = await uploadVoiceReference(caminho)
      await save({ ...settings, [campoDaVoz]: nome })
      setVoices(await listVoices())
    })

  return (
    <Section
      title="Voz"
      hint="A voz é CLONADA de um clipe seu, por um modelo que roda nesta máquina. Uns 10 segundos falando em português, sem música e sem ruído de fundo, já bastam. A primeira frase depois de abrir o app demora mais: é o modelo subindo para a placa de vídeo."
      error={error}
    >
      <div className="flex flex-wrap items-center gap-2">
        <Button onClick={cadastrar} disabled={isBusy}>
          Escolher clipe da minha voz…
        </Button>
        <Button
          variant="subtle"
          onClick={() => void run(() => speakText(TEST_PHRASE))}
          disabled={isBusy || !temVoz}
        >
          Testar voz
        </Button>
        <Button
          variant="subtle"
          onClick={() => void run(async () => setVoices(await listVoices()))}
          disabled={isBusy}
        >
          Carregar clipes
        </Button>
      </div>

      {!temVoz ? (
        <p className="text-muted text-[11px]">
          Sem clipe escolhido o Jarvis fica calado — inclusive no modo conversa, que recusa
          ligar.
        </p>
      ) : null}

      <label className="flex flex-col gap-1">
        {/* O clipe é UM POR TEMA: este campo edita o do tema ativo. Trocar de tema em
            Configurações traz o outro campo para cá, e cada um guarda o seu — sem isso,
            virar Ultron manteria a voz do Jarvis e a troca ficaria pela metade. */}
        <span className="text-muted text-[10px] tracking-[0.14em] uppercase">
          Voz do {NOME_DA_PERSONA[settings.persona]}
        </span>
        {/* Continua um campo de texto com `datalist`, e não um `<select>`, por um motivo
            que sobreviveu à troca de motor: listar os clipes exige o servidor de pé, e
            subir o servidor leva o tempo de carregar o modelo. Um select vazio enquanto
            isso deixaria a tela num beco — digitar o nome do arquivo continua valendo. */}
        <input
          type="text"
          list="tts-voices"
          value={vozAtual}
          onChange={(event) => void save({ ...settings, [campoDaVoz]: event.target.value })}
          placeholder="nome do clipe — vazio deixa o Jarvis mudo"
          spellCheck={false}
          className="border-border-soft bg-base text-content placeholder:text-muted/60 focus:border-accent rounded-lg border px-2 py-1.5 text-sm focus:outline-none"
        />
        <datalist id="tts-voices">
          {voices.map((voice) => (
            <option key={voice.id} value={voice.id}>
              {voice.name}
            </option>
          ))}
        </datalist>
      </label>
    </Section>
  )
}
