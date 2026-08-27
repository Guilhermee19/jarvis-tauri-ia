'use client'

import { useSensorStore, useSettingsStore } from '@/stores'
import { cn } from '@/lib/utils'

/**
 * A imagem da webcam como fundo da janela.
 *
 * Fica em `-z-20`, abaixo da grade e da vinheta do HUD: o efeito depende de a grade
 * passar POR CIMA da imagem, e não ao lado dela. A borda some por máscara radial
 * (`.webcam-stage`), então o que aparece nas quinas é o preto do `body` — não uma
 * moldura desenhada.
 */
export function WebcamStage() {
  const frame = useSensorStore((state) => state.webcamFrame)
  const isOn = useSensorStore((state) => state.isWebcamOn)
  // Espelhar em CSS e não no Rust: inverter lá obrigaria a decodificar e recodificar
  // cada quadro, e é justamente o passthrough de MJPEG que segura o custo do preview.
  const mirror = useSettingsStore((state) => state.settings.webcamMirror)

  if (!isOn) return null

  return (
    // A animação de entrada mora no wrapper, não na `<img>`: a imagem troca ~11×
    // por segundo, e ela reiniciaria a animação a cada quadro.
    <div className="webcam-stage pointer-events-none absolute inset-0 -z-20 overflow-hidden">
      {frame ? (
        // eslint-disable-next-line @next/next/no-img-element
        <img
          src={frame}
          alt=""
          aria-hidden
          className={cn('h-full w-full object-cover', mirror && '-scale-x-100')}
        />
      ) : null}

      {/* Linha de varredura: roda uma vez, na entrada, e some. */}
      <div className="webcam-scan bg-accent/70 absolute inset-x-0 top-0 h-px" />
    </div>
  )
}
