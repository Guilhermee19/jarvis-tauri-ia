'use client'

import { JarvisCore } from './JarvisCore'
import { useVoiceInput } from '@/hooks/useVoiceInput'
import { cn } from '@/lib/utils'
import { useJanelaStore, useSensorStore, useSettingsStore } from '@/stores'

/**
 * O HUD ocioso do assistente — o que fica na frente do fundo da janela.
 *
 * Com a webcam ligada o fundo deixa de ser a grade vazia e passa a ser a imagem da
 * câmera (`WebcamStage`). Por isso o núcleo encolhe e vai para o canto: centralizado
 * e grande, ele taparia justamente o meio do que a câmera está vendo.
 *
 * **O núcleo reage ao áudio**, e é o único sinal de microfone desta tela. Antes havia uma
 * linha de status dizendo "captando" ou "em espera"; ela mostrava se o microfone estava
 * ABERTO, e não se ele estava ouvindo alguma coisa — mudo no painel do Windows e
 * funcionando davam a mesma tela. O pulso mostra intensidade, que é a pergunta real.
 */
export function HomeScreen() {
  const assistantName = useSettingsStore((state) => state.settings.assistantName)
  const abrirJanela = useJanelaStore((state) => state.abrir)
  const isWebcamOn = useSensorStore((state) => state.isWebcamOn)
  const { nivelDeAudio } = useVoiceInput()

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

      </div>

      <SensorAlerts />
    </>
  )
}

/**
 * Permissão negada é o erro mais provável destes dois botões, e ele acontece longe
 * da bancada de diagnóstico — precisa aparecer aqui, onde o clique foi dado.
 */
function SensorAlerts() {
  const webcamError = useSensorStore((state) => state.webcamError)
  const micError = useSensorStore((state) => state.micError)

  if (!webcamError && !micError) return null

  return (
    <div className="absolute inset-x-3 bottom-3 flex flex-col gap-1.5">
      {webcamError ? <Alert label="Webcam" message={webcamError} /> : null}
      {micError ? <Alert label="Microfone" message={micError} /> : null}
    </div>
  )
}

function Alert({ label, message }: { label: string; message: string }) {
  return (
    <p className="border-danger/30 bg-danger/15 text-danger rounded border px-2 py-1.5 text-[11px] leading-relaxed backdrop-blur-sm">
      <span className="tracking-[0.14em] uppercase">{label}</span> · {message}
    </p>
  )
}
