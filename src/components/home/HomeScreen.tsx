'use client'

import { JarvisCore } from './JarvisCore'
import { useChatStore, useSettingsStore, useSheetStore } from '@/stores'

/**
 * O HUD ocioso do assistente — fundo permanente da janela, sob os painéis.
 *
 * O núcleo é a única coisa clicável (abre a janelinha de conversa) e as linhas de
 * status mostram o estado de cada subsistema. "Diagnóstico" quer dizer que a
 * capacidade existe e é testável na bancada, mas ainda não está ligada ao agente;
 * "offline" continua sendo um módulo Rust vazio esperando a versão que o implementa.
 */
export function HomeScreen() {
  const assistantName = useSettingsStore((state) => state.settings.assistantName)
  const hasApiKey = useSettingsStore((state) => state.settings.anthropicApiKey.length > 0)
  const messageCount = useChatStore((state) => state.messages.length)
  const openSheet = useSheetStore((state) => state.open)

  return (
    <div className="no-select absolute inset-0 flex flex-col items-center justify-center px-6">
      <button
        type="button"
        onClick={() => openSheet('chat')}
        title="Abrir a conversa"
        className="text-accent relative transition-transform duration-300 hover:scale-[1.03] focus:outline-none"
      >
        <JarvisCore label={assistantName} className="h-64 w-64" />
      </button>

      <p className="text-muted mt-9 text-[10px] tracking-[0.28em] uppercase">
        {messageCount > 0 ? `${messageCount} mensagens na sessão` : 'toque para conversar'}
      </p>

      <dl className="mt-8 w-full max-w-[300px] space-y-1.5">
        <StatusRow label="Núcleo" value="simulado" tone="warn" />
        <StatusRow label="Voz" value="diagnóstico" tone="warn" />
        <StatusRow label="Visão" value="diagnóstico" tone="warn" />
        <StatusRow label="Memória" value="sessão" />
        <StatusRow
          label="API key"
          value={hasApiKey ? 'definida' : 'ausente'}
          tone={hasApiKey ? 'ok' : 'warn'}
        />
      </dl>
    </div>
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
