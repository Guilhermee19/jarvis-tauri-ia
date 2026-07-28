'use client'

import { JarvisCore } from './JarvisCore'
import { cn } from '@/lib/utils'
import { useChatStore, useSensorStore, useSettingsStore, useSheetStore } from '@/stores'

/**
 * O HUD ocioso do assistente — o que fica na frente do fundo da janela.
 *
 * Com a webcam ligada o fundo deixa de ser a grade vazia e passa a ser a imagem da
 * câmera (`WebcamStage`). Por isso o núcleo encolhe e vai para o canto: centralizado
 * e grande, ele taparia justamente o meio do que a câmera está vendo.
 */
export function HomeScreen() {
  const assistantName = useSettingsStore((state) => state.settings.assistantName)
  const hasApiKey = useSettingsStore((state) => state.settings.anthropicApiKey.length > 0)
  const messageCount = useChatStore((state) => state.messages.length)
  const openSheet = useSheetStore((state) => state.open)
  const isWebcamOn = useSensorStore((state) => state.isWebcamOn)
  const isMicOn = useSensorStore((state) => state.isMicOn)

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
          onClick={() => openSheet('chat')}
          title="Abrir a conversa"
          className="text-accent relative transition-transform duration-300 hover:scale-[1.03] focus:outline-none"
        >
          <JarvisCore
            label={assistantName}
            className={cn('transition-all duration-500', isWebcamOn ? 'h-24 w-24' : 'h-64 w-64')}
          />
        </button>

        {/* Com a câmera ligada, o texto de apoio sai de cena: o conteúdo é a imagem. */}
        {isWebcamOn ? null : (
          <>
            <p className="text-muted mt-9 text-[10px] tracking-[0.28em] uppercase">
              {messageCount > 0 ? `${messageCount} mensagens na sessão` : 'toque para conversar'}
            </p>

            <dl className="mt-8 w-full max-w-[300px] space-y-1.5">
              <StatusRow label="Núcleo" value="simulado" tone="warn" />
              <StatusRow
                label="Voz"
                value={isMicOn ? 'captando' : 'em espera'}
                tone={isMicOn ? 'ok' : 'idle'}
              />
              <StatusRow label="Visão" value="em espera" />
              <StatusRow label="Memória" value="sessão" />
              <StatusRow
                label="API key"
                value={hasApiKey ? 'definida' : 'ausente'}
                tone={hasApiKey ? 'ok' : 'warn'}
              />
            </dl>
          </>
        )}
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

const TONES = {
  idle: 'text-muted',
  ok: 'text-accent',
  warn: 'text-content',
} as const

function StatusRow({
  label,
  value,
  tone = 'idle',
}: {
  label: string
  value: string
  tone?: keyof typeof TONES
}) {
  return (
    <div className="flex items-baseline gap-2 text-[10px] tracking-[0.14em] uppercase">
      <dt className="text-muted/70">{label}</dt>
      {/* A linha pontilhada liga rótulo e valor sem precisar de tabela. */}
      <div className="border-border-soft mb-[3px] flex-1 border-b border-dotted" />
      <dd className={TONES[tone]}>{value}</dd>
    </div>
  )
}
