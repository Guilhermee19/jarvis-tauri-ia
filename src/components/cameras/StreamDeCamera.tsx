'use client'

import { useState } from 'react'

import { cn } from '@/lib/utils'
import { useCamerasStore } from '@/stores'
import type { Camera } from '@/types'

/**
 * A imagem ao vivo de uma câmera.
 *
 * **MP4 progressivo numa `<video>`.** O go2rtc serve `/api/stream.mp4` como fMP4 em
 * `Transfer-Encoding: chunked`, que o navegador toca nativamente — sem script externo,
 * sem MSE na mão, sem custom element (que exigiria declaração de JSX no TypeScript) e
 * sem nenhuma dependência nova no `package.json`.
 *
 * **Não é MJPEG, e isso foi medido.** O caminho óbvio seria `/api/stream.mjpeg` numa
 * `<img>`, no mesmo molde da webcam. Ele responde `200` com **`Content-Length: 0`**:
 * produzir MJPEG a partir de H.264 exige transcodificação, que o go2rtc só faz com um
 * ffmpeg configurado. O modo de falha é cruel — status de sucesso, corpo vazio, nenhum
 * erro em lugar nenhum, só um quadro que nunca aparece.
 *
 * O MP4 sai melhor de qualquer forma: é o H.264 da câmera repassado sem recodificar, o
 * que custa menos CPU no go2rtc e menos banda que JPEG quadro a quadro.
 */
export function StreamDeCamera({ camera, className }: { camera: Camera; className?: string }) {
  const baseUrl = useCamerasStore((state) => state.baseUrl)
  const quadro = useCamerasStore((state) => state.quadros[camera.id])
  const atualizarQuadro = useCamerasStore((state) => state.atualizarQuadro)

  // A identidade do fluxo atual. Trocar de câmera, ou o serviço reiniciar, faz outra.
  const fluxo = `${camera.id}@${baseUrl}`
  /**
   * Qual fluxo falhou — e não um booleano.
   *
   * É o que faz uma câmera que voltou a existir ganhar nova tentativa **sem um
   * `useEffect` que zera o estado**: comparando durante o render, a falha do fluxo
   * anterior simplesmente deixa de valer para o novo. Um `setState` dentro de efeito
   * para sincronizar isso é o padrão que o React desaconselha, e aqui ele nem é preciso.
   */
  const [fluxoQueFalhou, setFluxoQueFalhou] = useState<string | null>(null)
  const falhou = fluxoQueFalhou === fluxo

  // Sem o serviço de pé não há o que mostrar, e um `src` vazio pisca um elemento quebrado.
  if (!baseUrl) {
    return <Aviso className={className}>Ligando o serviço de vídeo…</Aviso>
  }

  if (falhou) {
    // Degradação graciosa: o fluxo caiu, mas um quadro parado ainda diz se a câmera está
    // viva. É o mesmo `data:` URL que a visão consome, pelo `/api/frame.jpeg`.
    return (
      <div className={cn('relative', className)}>
        {quadro ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img src={quadro} alt={camera.nome} className="h-full w-full object-cover opacity-70" />
        ) : (
          <Aviso className="h-full w-full">Sem imagem.</Aviso>
        )}
        <button
          type="button"
          onClick={() => {
            void atualizarQuadro(camera.id)
            setFluxoQueFalhou(null)
          }}
          className="bg-base/80 border-border-soft text-muted hover:text-content absolute right-2 bottom-2 rounded-md border px-2 py-1 text-[10px]"
        >
          Tentar de novo
        </button>
      </div>
    )
  }

  return (
    <video
      // A `key` força um elemento novo quando a câmera muda: reusar o `<video>` manteria
      // a conexão chunked anterior aberta, e o go2rtc contaria dois consumidores do
      // mesmo stream para sempre.
      key={fluxo}
      src={`${baseUrl}/api/stream.mp4?src=${encodeURIComponent(camera.id)}`}
      // `muted` não é preferência: sem ele o navegador recusa o autoplay e o quadro fica
      // parado no primeiro frame. `playsInline` evita o player em tela cheia.
      autoPlay
      muted
      playsInline
      // Nada de `controls`: numa grade de quatro, quatro barras de player competem com a
      // imagem — e não há o que controlar num fluxo ao vivo sem linha do tempo.
      onError={() => {
        setFluxoQueFalhou(fluxo)
        // Pede um quadro parado para ter o que mostrar no lugar. Se este também falhar, a
        // store engole em silêncio e cai no "Sem imagem".
        void atualizarQuadro(camera.id)
      }}
      className={cn('bg-base/60 object-cover', className)}
    />
  )
}

function Aviso({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div
      className={cn(
        'bg-base/60 text-muted flex items-center justify-center p-4 text-center text-[11px]',
        className,
      )}
    >
      {children}
    </div>
  )
}
