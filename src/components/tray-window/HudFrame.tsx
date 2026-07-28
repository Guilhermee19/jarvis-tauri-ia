/**
 * Moldura do HUD: borda fina com colchetes nos cantos, por cima de tudo.
 *
 * É puramente decorativa — `pointer-events-none` garante que não roube clique de
 * nada, inclusive da região de arrasto da janela.
 */
export function HudFrame() {
  return (
    <div className="pointer-events-none absolute inset-0 z-20">
      <div className="border-accent/10 absolute inset-1.5 border" />

      <span className="border-accent/40 absolute top-1.5 left-1.5 h-3.5 w-3.5 border-t border-l" />
      <span className="border-accent/40 absolute top-1.5 right-1.5 h-3.5 w-3.5 border-t border-r" />
      <span className="border-accent/40 absolute bottom-1.5 left-1.5 h-3.5 w-3.5 border-b border-l" />
      <span className="border-accent/40 absolute right-1.5 bottom-1.5 h-3.5 w-3.5 border-r border-b" />
    </div>
  )
}
