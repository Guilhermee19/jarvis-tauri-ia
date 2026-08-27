'use client'

import { cn } from '@/lib/utils'

interface SectionProps {
  title: string
  /** Uma linha dizendo o que o teste prova — a tela é de diagnóstico, não de uso. */
  hint: string
  error: string | null
  children: React.ReactNode
}

export function Section({ title, hint, error, children }: SectionProps) {
  return (
    <section className="border-border-soft bg-base/40 rounded-lg border p-3">
      <h3 className="text-content text-[11px] tracking-[0.18em] uppercase">{title}</h3>
      <p className="text-muted mt-1 text-[11px] leading-relaxed">{hint}</p>

      <div className="mt-3 flex flex-col gap-2">{children}</div>

      {error ? (
        <p className="border-danger/30 bg-danger/10 text-danger mt-3 rounded border px-2 py-1.5 text-[11px] leading-relaxed">
          {error}
        </p>
      ) : null}
    </section>
  )
}

/**
 * Moldura das imagens capturadas: mantém proporção sem estourar a gaveta.
 *
 * `mirror` é só da webcam, e é transformação de EXIBIÇÃO — a imagem por baixo não
 * muda. Vale para o preview e para o frame capturado pelos dois estarem na mesma
 * tela: espelhar um e o outro não faria o botão parecer que capturou outra coisa.
 */
export function Preview({
  src,
  label,
  mirror = false,
}: {
  src: string
  label: string
  mirror?: boolean
}) {
  return (
    // `next/image` não tem o que otimizar aqui: a fonte é uma `data:` URL vinda do
    // Rust, o app é export estático e não há servidor para redimensionar nada.
    // eslint-disable-next-line @next/next/no-img-element
    <img
      src={src}
      alt={label}
      className={cn(
        'border-border-soft bg-base w-full rounded border object-contain',
        mirror && '-scale-x-100',
      )}
    />
  )
}
