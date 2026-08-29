'use client'

import { useState } from 'react'
import { Section } from './Section'
import { Button } from '@/components/ui/Button'
import { useAsyncAction } from '@/hooks/useAsyncAction'
import { escolherClipeDeVoz, listVoices, speakText, uploadVoiceReference } from '@/lib/tauri'
import { useSettingsStore } from '@/stores'
import {
  campoDaVoz,
  NOME_DA_PERSONA,
  VOZES_PIPER,
  vozDaPersona,
  type MotorDeVoz,
  type Voice,
} from '@/types'

/** Frase fixa do teste: curta, em português, e com número para checar a prosódia. */
const TEST_PHRASE = 'Sistemas online. Sou o Jarvis, e estou ouvindo você.'

/** A mesma string dos quatro `<select>` do SettingsForm — o projeto não tem componente. */
const SELECT =
  'border-border-soft bg-base text-content focus:border-accent w-full rounded-lg border px-3 py-2 text-sm focus:outline-none'

export function SpeechSection() {
  const settings = useSettingsStore((state) => state.settings)
  const save = useSettingsStore((state) => state.save)
  const [voices, setVoices] = useState<Voice[]>([])
  const { isBusy, error, run } = useAsyncAction()

  // Qual campo editar sai de `campoDaVoz`, que cruza motor × persona no mesmo lugar que o
  // `voz()` do Rust. Repetir esse cruzamento aqui já deu errado uma vez.
  const campo = campoDaVoz(settings)
  const vozAtual = vozDaPersona(settings)
  const temVoz = vozAtual.trim().length > 0
  const usaPiper = settings.ttsEngine === 'piper'

  const guardar = (voz: string) => void save({ ...settings, [campo]: voz })

  /** Escolhe um arquivo, manda para o servidor, e guarda o nome que ele devolveu. */
  const cadastrar = () =>
    void run(async () => {
      const caminho = await escolherClipeDeVoz()
      if (!caminho) return

      // O nome vem do servidor, e não do caminho escolhido: ele higieniza o nome do
      // arquivo, e guardar o nosso daria uma voz que não existe do lado de lá.
      const nome = await uploadVoiceReference(caminho)
      await save({ ...settings, [campo]: nome })
      setVoices(await listVoices())
    })

  return (
    <Section
      title="Voz"
      hint={
        usaPiper
          ? 'O Piper roda na CPU e deixa a placa de vídeo inteira para o Ollama. As vozes são de catálogo — não são a sua, mas respondem numa fração do tempo.'
          : 'O Chatterbox CLONA a sua voz de um clipe de uns 10 segundos, mas gera mais devagar que o próprio áudio: conte alguns segundos por frase. A primeira depois de abrir o app demora mais, que é o modelo subindo para a placa.'
      }
      error={error}
    >
      <label className="flex flex-col gap-1">
        <span className="text-muted text-[10px] tracking-[0.14em] uppercase">Motor</span>
        <select
          value={settings.ttsEngine}
          onChange={(event) => void save({ ...settings, ttsEngine: event.target.value as MotorDeVoz })}
          className={SELECT}
        >
          <option value="piper">Piper — rápido, voz de catálogo</option>
          <option value="chatterbox">Chatterbox — a sua voz clonada, mais lento</option>
        </select>
        {/* Cada motor guarda a SUA voz: trocar aqui não perde o que estava escolhido no
            outro, e voltar traz de volta. */}
        <p className="text-muted text-[11px] leading-snug">
          Cada motor guarda a própria voz, e cada tema também — trocar aqui não apaga o que
          você escolheu no outro.
        </p>
      </label>

      <div className="flex flex-wrap items-center gap-2">
        {!usaPiper ? (
          <Button onClick={cadastrar} disabled={isBusy}>
            Escolher clipe da minha voz…
          </Button>
        ) : null}
        <Button
          variant={usaPiper ? undefined : 'subtle'}
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
          Conferir instaladas
        </Button>
      </div>

      {!temVoz ? (
        <p className="text-muted text-[11px]">
          Sem voz escolhida o Jarvis fica calado — inclusive no modo conversa, que recusa
          ligar.
        </p>
      ) : null}

      <label className="flex flex-col gap-1">
        {/* A voz é UMA POR TEMA: este campo edita a do tema ativo. Trocar de tema em
            Configurações traz o outro campo para cá, e cada um guarda o seu — sem isso,
            virar Ultron manteria a voz do Jarvis e a troca ficaria pela metade. */}
        <span className="text-muted text-[10px] tracking-[0.14em] uppercase">
          Voz do {NOME_DA_PERSONA[settings.persona]}
        </span>

        {usaPiper ? (
          <select value={vozAtual} onChange={(event) => guardar(event.target.value)} className={SELECT}>
            <option value="">escolha uma voz…</option>
            {VOZES_PIPER.map((voz) => (
              <option key={voz.id} value={voz.id}>
                {voz.nome}
              </option>
            ))}
          </select>
        ) : (
          /* Campo de texto com `datalist`, e não um `<select>`: listar os clipes exige o
             servidor de pé, e subir o Chatterbox leva o tempo de carregar o modelo. Um
             select vazio enquanto isso deixaria a tela num beco — digitar o nome do
             arquivo continua valendo. */
          <input
            type="text"
            list="tts-voices"
            value={vozAtual}
            onChange={(event) => guardar(event.target.value)}
            placeholder="nome do clipe — vazio deixa o Jarvis mudo"
            spellCheck={false}
            className="border-border-soft bg-base text-content placeholder:text-muted/60 focus:border-accent rounded-lg border px-2 py-1.5 text-sm focus:outline-none"
          />
        )}

        <datalist id="tts-voices">
          {voices.map((voice) => (
            <option key={voice.id} value={voice.id}>
              {voice.name}
            </option>
          ))}
        </datalist>
      </label>

      {/* Mesmo aviso que o SettingsForm dá para microfone e webcam: um valor salvo que não
          está mais na lista precisa aparecer, senão o select mostra vazio e some com a
          informação de que havia algo escolhido. */}
      {usaPiper && temVoz && !VOZES_PIPER.some((voz) => voz.id === vozAtual) ? (
        <p className="text-danger text-[11px] leading-snug">
          A voz salva ({vozAtual}) não é uma das do catálogo. O Piper não recusa voz
          desconhecida — ele usa a padrão em silêncio, então escolha uma da lista.
        </p>
      ) : null}

      {voices.length > 0 ? (
        <p className="text-muted text-[11px] leading-snug">
          Instaladas no servidor: {voices.map((voice) => voice.name).join(', ')}.
        </p>
      ) : null}
    </Section>
  )
}
