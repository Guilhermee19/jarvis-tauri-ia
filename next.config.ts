import type { NextConfig } from 'next'

const nextConfig: NextConfig = {
  // Tauri serve a UI como arquivos estáticos: não existe servidor Node no bundle,
  // então tudo precisa sair pré-renderizado em `out/` (ver `frontendDist` no tauri.conf.json).
  output: 'export',
  // O otimizador de imagens do Next depende do servidor — incompatível com `output: 'export'`.
  images: { unoptimized: true },
  // O badge de dev do Next flutua por cima da barra de título da janela.
  devIndicators: false,
}

export default nextConfig
