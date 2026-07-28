'use client'

import { ChatWindow } from '@/components/chat/ChatWindow'
import { HomeScreen } from '@/components/home/HomeScreen'
import { DiagnosticsSheet } from '@/components/sheets/DiagnosticsSheet'
import { SettingsSheet } from '@/components/sheets/SettingsSheet'
import { BottomNav } from '@/components/tray-window/BottomNav'
import { HudFrame } from '@/components/tray-window/HudFrame'
import { TitleBar } from '@/components/tray-window/TitleBar'
import { useBootstrap } from '@/hooks/useBootstrap'
import { useTrayEvents } from '@/hooks/useTrayEvents'

export default function HomePage() {
  useBootstrap()
  useTrayEvents()

  return (
    // Sem `bg-base` aqui de propósito: o fundo do `main` seria pintado por cima
    // das camadas com z-index negativo, apagando a grade. A cor vem do `body`.
    <main className="relative flex h-screen flex-col overflow-hidden">
      {/* Fundo do HUD: vale para toda a janela, por isso mora no shell. */}
      <div className="hud-grid pointer-events-none absolute inset-0 -z-10" />
      <div className="hud-vignette pointer-events-none absolute inset-0 -z-10" />

      <TitleBar />

      {/* O HUD é o fundo permanente; os painéis de feature ficam por cima dele.
          Este container é o limite deles: assim não cobrem a barra de título nem a
          de ícones, e é dentro dele que a janelinha do chat pode ser arrastada. */}
      <div className="relative min-h-0 flex-1">
        <HomeScreen />
        <ChatWindow />
        <DiagnosticsSheet />
        <SettingsSheet />
      </div>

      <BottomNav />
      <HudFrame />
    </main>
  )
}
