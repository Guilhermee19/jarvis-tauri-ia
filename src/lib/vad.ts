/**
 * Quando a sua frase acabou — a decisão que fecha um turno do modo conversa.
 *
 * O sinal é o pico do microfone que o Rust já publica em `jarvis://mic-level` 20×/s.
 * Nada aqui abre dispositivo, chama Tauri ou toca em React: é aritmética sobre
 * (nível, relógio), justamente para poder ser testada sem microfone (`vad.test.ts`).
 *
 * ponytail: VAD por ENERGIA, não por modelo de fala. Ventilador, TV e a voz de
 * outra pessoa na sala passam do limiar igual. Trocar por webrtc-vad/Silero dentro
 * do `mic.rs` se o falso-positivo incomodar — a assinatura daqui não muda.
 */

/**
 * Acima disso é fala. Um degrau acima do `PICO_MINIMO = 0.015` do `stt.rs`, que é o
 * ponto em que o Whisper já considera a gravação silêncio e recusa transcrever:
 * abaixo do que ele aceita, fechar o turno só produziria uma ida e volta vazia.
 */
export const LIMIAR = 0.02

/**
 * Silêncio que encerra a frase. Abaixo de ~1 s ele corta quem pensa no meio da
 * oração ("abre o... youtube"); muito acima, cada resposta demora um tempo a mais
 * que o usuário sente como travamento.
 */
export const SILENCIO_MS = 1200

/**
 * Sem nunca ter ouvido nada, a gravação é jogada fora e recomeçada. O `Recorder`
 * acumula as amostras em memória até o `stop`, então o modo conversa esquecido
 * ligado cresceria sem teto — 30 s de silêncio é lixo de qualquer jeito.
 */
export const OCIOSO_MS = 30_000

export interface TurnoVad {
  /** Instante do último pico acima do limiar, ou `null` se ainda não falou nada. */
  falouEm: number | null
  /** Quando esta gravação abriu — a base do ocioso. */
  desde: number
}

export type DecisaoVad = 'ouvindo' | 'fechar' | 'reciclar'

export function iniciarTurno(agora: number): TurnoVad {
  return { falouEm: null, desde: agora }
}

/**
 * Devolve a decisão E o turno atualizado, em vez de só a decisão: o carimbo do
 * último pico é parte da regra, e deixá-lo para o chamador seria a mesma lógica
 * escrita em dois lugares — um deles sem teste.
 */
export function avaliarTurno(
  turno: TurnoVad,
  level: number,
  agora: number,
): { decisao: DecisaoVad; turno: TurnoVad } {
  if (level > LIMIAR) {
    return { decisao: 'ouvindo', turno: { ...turno, falouEm: agora } }
  }

  if (turno.falouEm !== null) {
    const decisao = agora - turno.falouEm >= SILENCIO_MS ? 'fechar' : 'ouvindo'
    return { decisao, turno }
  }

  const decisao = agora - turno.desde >= OCIOSO_MS ? 'reciclar' : 'ouvindo'
  return { decisao, turno }
}
