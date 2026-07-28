'use client'

import { MicSection } from './MicSection'
import { ScreenSection } from './ScreenSection'
import { SpeechSection } from './SpeechSection'
import { WebcamSection } from './WebcamSection'

/**
 * Bancada de testes das capacidades de entrada e saída da v0.1.5.
 *
 * Cada seção exercita UM comando do backend de forma isolada — é o lugar de
 * descobrir que o microfone está mudo ou a câmera bloqueada sem envolver o chat.
 * Quando o agente chegar (v0.2+), ele chama exatamente os mesmos comandos.
 */
export function DiagnosticsPanel() {
  return (
    <div className="scroll-thin flex flex-1 flex-col gap-3 overflow-y-auto p-3">
      <MicSection />
      <SpeechSection />
      <WebcamSection />
      <ScreenSection />
    </div>
  )
}
