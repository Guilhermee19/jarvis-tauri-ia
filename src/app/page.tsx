'use client'

import { ChatWindow } from '@/components/chat/ChatWindow'
import { HomeScreen } from '@/components/home/HomeScreen'
import { NowPlayingWidget } from '@/components/music/NowPlayingWidget'
import { WebcamStage } from '@/components/home/WebcamStage'
import { CamerasWindow } from '@/components/cameras/CamerasWindow'
import { CasaWindow } from '@/components/casa/CasaWindow'
import { CotacoesWindow } from '@/components/cotacoes/CotacoesWindow'
import { DesempenhoWindow } from '@/components/desempenho/DesempenhoWindow'
import { ConhecimentoWindow } from '@/components/conhecimento/ConhecimentoWindow'
import { NavegadorWindow } from '@/components/navegador/NavegadorWindow'
import { SettingsSheet } from '@/components/sheets/SettingsSheet'
import { BottomNav } from '@/components/tray-window/BottomNav'
import { HudFrame } from '@/components/tray-window/HudFrame'
import { TitleBar } from '@/components/tray-window/TitleBar'
import { useBootstrap } from '@/hooks/useBootstrap'
import { useNowPlaying } from '@/hooks/useNowPlaying'
import { usePersona } from '@/hooks/usePersona'
import { useSaudacao } from '@/hooks/useSaudacao'
import { useSensorEvents } from '@/hooks/useSensorEvents'
import { useTrayEvents } from '@/hooks/useTrayEvents'

export default function HomePage() {
  useBootstrap()
  usePersona()
  useTrayEvents()
  useSensorEvents()
  useNowPlaying()
  // Depois do `useSensorEvents`: a saudação pode pedir o cadastro de um rosto, e quem
  // escuta esse pedido é o hook de eventos — que precisa já estar assinado.
  useSaudacao()

  return (
    // Sem `bg-base` aqui de propósito: o fundo do `main` seria pintado por cima
    // das camadas com z-index negativo, apagando a grade. A cor vem do `body`.
    <main className="relative flex h-screen flex-col overflow-hidden">
      {/* Fundo do HUD: vale para toda a janela, por isso mora no shell. A webcam
          entra ABAIXO da grade (-z-20) — o efeito é a grade passando por cima da
          imagem, como se a tela fosse um visor. */}
      <WebcamStage />
      <div className="hud-grid pointer-events-none absolute inset-0 -z-10" />
      <div className="hud-vignette pointer-events-none absolute inset-0 -z-10" />

      <TitleBar />

      {/* O HUD é o fundo permanente; os painéis de feature ficam por cima dele.
          Este container é o limite deles: assim não cobrem a barra de título nem a
          de ícones, e é dentro dele que a janelinha do chat pode ser arrastada. */}
      <div className="relative min-h-0 flex-1">
        <HomeScreen />
        <ChatWindow />
        <CasaWindow />
        <CamerasWindow />
        <DesempenhoWindow />
        <CotacoesWindow />
        <NavegadorWindow />
        <ConhecimentoWindow />
        <SettingsSheet />
        <NowPlayingWidget />
      </div>

      <BottomNav />
      <HudFrame />
    </main>
  )
}
