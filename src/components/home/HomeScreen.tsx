'use client'

import { JarvisCore } from './JarvisCore'
import { useVoiceInput, type EscutaStatus as EscutaStatusType } from '@/hooks/useVoiceInput'
import { cn } from '@/lib/utils'
import { useJanelaStore, useSensorStore, useSettingsStore } from '@/stores'

/**
 * O HUD ocioso do assistente — o que fica na frente do fundo da janela.
 *
 * Com a webcam ligada o fundo deixa de ser a grade vazia e passa a ser a imagem da
 * câmera (`WebcamStage`). Por isso o núcleo encolhe e vai para o canto: centralizado
 * e grande, ele taparia justamente o meio do que a câmera está vendo.
 *
 * **O núcleo reage ao áudio**, e é o principal sinal de microfone desta tela. Antes havia
 * uma linha de status dizendo "captando" ou "em espera"; ela mostrava se o microfone estava
 * ABERTO, e não se ele estava ouvindo alguma coisa — mudo no painel do Windows e
 * funcionando davam a mesma tela. O pulso mostra intensidade, que é a pergunta real.
 *
 * O rótulo abaixo dele responde outra pergunta, que o pulso não responde: de quem é a vez.
 * Só aparece com a escuta ligada, e só então, porque enquanto ele pensa ou fala o
 * microfone está fechado de propósito — e um núcleo parado nesses segundos seria lido
 * como travamento.
 */
export function HomeScreen() {
  const assistantName = useSettingsStore((state) => state.settings.assistantName)
  const abrirJanela = useJanelaStore((state) => state.abrir)
  const isWebcamOn = useSensorStore((state) => state.isWebcamOn)
  const { nivelDeAudio, isListening, status } = useVoiceInput()

  // A raiz quadrada tira a fala do fundo da escala linear — é a mesma curva das três
  // barras de nível que já existem no app, e o motivo está no `BottomNav`.
  const pulso = Math.sqrt(nivelDeAudio)

  return (
    <>
      <div
        className={cn(
          'no-select absolute flex flex-col items-center',
          isWebcamOn ? 'right-4 bottom-4' : 'inset-0 justify-center px-6',
        )}
      >
        <button
          type="button"
          onClick={() => abrirJanela('chat')}
          title="Abrir a conversa"
          className="text-accent relative transition-transform duration-300 hover:scale-[1.03] focus:outline-none"
        >
          <JarvisCore
            label={assistantName}
            nivel={pulso}
            className={cn('transition-all duration-500', isWebcamOn ? 'h-24 w-24' : 'h-64 w-64')}
          />
        </button>

        {isListening ? <EscutaStatus status={status} assistantName={assistantName} /> : null}
      </div>

      <SensorAlerts />
    </>
  )
}

/** O que dizer em cada palavra do turno — o "ouvindo" é o único que ensina o gatilho. */
const ROTULOS = {
  ouvindo: 'Ouvindo',
  pensando: 'Pensando',
  falando: 'Falando',
} as const

function EscutaStatus({
  status,
  assistantName,
}: {
  status: EscutaStatusType
  assistantName: string
}) {
  return (
    <p className="text-muted mt-3 text-center text-[10px] tracking-[0.14em] uppercase">
      {ROTULOS[status]}
      {status === 'ouvindo' ? (
        // Sem isto, o microfone ligado promete mais do que cumpre: ele ouve tudo, mas
        // só obedece ao que vem depois do nome — e essa regra não tem outro lugar
        // onde apareça na hora certa.
        <span className="text-muted/60 normal-case"> · diga “{assistantName}, …”</span>
      ) : null}
    </p>
  )
}

/**
 * Permissão negada é o erro mais provável destes botões, e ele acontece longe
 * da bancada de diagnóstico — precisa aparecer aqui, onde o clique foi dado.
 *
 * O erro da escuta entrou nesta lista quando o botão de falar saiu do chat: ele
 * aparecia ao lado daquele botão, e sem ele a falha do microfone não tinha mais onde
 * ser vista. As mensagens do Rust já dizem o que fazer ("baixe o whisper-blas-bin-x64.zip…",
 * "Configurações › Privacidade › Microfone"), só precisam de tela.
 */
function SensorAlerts() {
  const webcamError = useSensorStore((state) => state.webcamError)
  const micError = useSensorStore((state) => state.micError)
  const escutaError = useSensorStore((state) => state.dictationError)
  const limparEscutaError = useSensorStore((state) => state.clearDictationError)

  if (!webcamError && !micError && !escutaError) return null

  return (
    <div className="absolute inset-x-3 bottom-3 flex flex-col gap-1.5">
      {webcamError ? <Alert label="Webcam" message={webcamError} /> : null}
      {micError ? <Alert label="Microfone" message={micError} /> : null}
      {escutaError ? (
        <Alert label="Escuta" message={escutaError} onDismiss={limparEscutaError} />
      ) : null}
    </div>
  )
}

function Alert({
  label,
  message,
  onDismiss,
}: {
  label: string
  message: string
  onDismiss?: () => void
}) {
  return (
    <p className="border-danger/30 bg-danger/15 text-danger flex items-start gap-2 rounded border px-2 py-1.5 text-[11px] leading-relaxed backdrop-blur-sm">
      <span className="flex-1 whitespace-pre-line">
        <span className="tracking-[0.14em] uppercase">{label}</span> · {message}
      </span>
      {/* Só o erro da escuta se dispensa: os outros dois somem sozinhos ao ligar o
          sensor de novo, enquanto este fica até alguém lê-lo — o turno que falhou
          já acabou. */}
      {onDismiss ? (
        <button
          type="button"
          onClick={onDismiss}
          aria-label={`Dispensar o aviso de ${label.toLowerCase()}`}
          className="text-danger/70 hover:text-danger shrink-0 leading-none"
        >
          ✕
        </button>
      ) : null}
    </p>
  )
}
