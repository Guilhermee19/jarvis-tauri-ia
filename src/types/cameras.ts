/**
 * As câmeras de segurança da casa. Espelho manual de `core::cameras` no Rust.
 *
 * Espelho manual como o resto de `src/types/` — o projeto não usa `ts-rs`. Campo novo
 * no struct do Rust precisa ser copiado aqui, senão ele atravessa o IPC e some sem
 * erro nenhum.
 */

/**
 * Que dialeto a câmera fala — decide como a URL do stream é montada e se ela se mexe.
 *
 * `dvr` é o Xiongmai que o app XMEye abre: um endereço com vários canais, sem PTZ por
 * este caminho. `onvif` é a câmera que responde por conta própria onde está o stream,
 * como a V380, e costuma aceitar pan/tilt.
 */
export type TipoDeCamera = 'dvr' | 'onvif'

/** Espelha `core::cameras::Camera`. */
export interface Camera {
  /** Identificador estável, e é ele que vira o `src` do go2rtc. Sem espaço nem acento. */
  id: string
  /** Como você a chama em voz alta: "garagem", "portão". */
  nome: string
  /** IP na rede local, sem porta. */
  host: string
  tipo: TipoDeCamera
  /** Qual câmera dentro do DVR, começando em 1. Ignorado quando o tipo é `onvif`. */
  canal: number
  usuario: string
  senha: string
  /**
   * A URL RTSP crua, quando ela não pode ser derivada.
   *
   * **Vazio é o caso normal**, não um cadastro pela metade: para o DVR a URL sai do
   * host, canal e credenciais. Preenchido, ganha do palpite — é onde vai o que o
   * {@link probeCamera} descobriu.
   */
  rtspUrl: string
  /** Fora da grade principal, por escolha. Ela continua atendendo por voz. */
  oculto: boolean
  /** Vigiar esta câmera em busca de movimento. */
  vigiar: boolean
}

/** O que `start_cameras` devolve. Espelha `commands::cameras::Ligado`. */
export interface CamerasLigadas {
  /**
   * A base do go2rtc (`http://127.0.0.1:8646`).
   *
   * Vem do Rust em vez de ser uma constante daqui porque a porta mora lá — duas
   * constantes dizendo a mesma coisa é uma que fica velha.
   */
  baseUrl: string
  cameras: Camera[]
}

/** O que uma sondagem ONVIF descobriu. Espelha `commands::cameras::Sondagem`. */
export interface Sondagem {
  /** "IPCAM IPCAM (HS-Camera_No1)" — o bastante para confirmar que é a câmera certa. */
  descricao: string
  /** A URL que a própria câmera disse, pronta para o cadastro. */
  rtspUrl: string
  perfis: string[]
}

/** Para onde a câmera vira. Espelha `core::cameras::onvif::Direcao`. */
export type Direcao = 'esquerda' | 'direita' | 'cima' | 'baixo'

/**
 * Uma câmera encontrada pela varredura da rede. Espelha
 * `core::cameras::varredura::Achado`.
 */
export interface Achado {
  host: string
  tipo: TipoDeCamera
  /** "IPCAM (HS-Camera_No1)", "DVR Xiongmai / XMEye (H264DVR 1.0)". */
  descricao: string
  /** A URL que o ONVIF entregou. Vazia no DVR, onde ela é derivada do canal. */
  rtspUrl: string
  /** Pediu autenticação — sem usuário e senha, esse cadastro não mostra imagem. */
  precisaSenha: boolean
  /** Já está no catálogo. Continua na lista, marcado: sumir pareceria que não foi achada. */
  jaCadastrada: boolean
}
