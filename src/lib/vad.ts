/**
 * Quando a sua frase acabou — a decisão que fecha um turno da escuta contínua.
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
 * O limiar nunca desce abaixo disto, e o número é **de propósito menor** que o
 * `PICO_MINIMO = 0.015` do `stt.rs`.
 *
 * Já foi 0,02, um degrau ACIMA daquele — e era o degrau errado. O `stt` recusa a
 * gravação cujo pico não chega a 0,015; se o VAD só reconhece fala a partir de 0,02,
 * existe uma faixa inteira de voz que o Whisper transcreveria de boa vontade e que
 * nunca chega até ele, porque o turno não fecha. Quem fala baixo, ou está a dois
 * metros do microfone, mora nessa faixa: a frase some sem erro nenhum na tela, e a
 * saída que sobra é gritar. Ficando abaixo do portão do `stt`, o que o Whisper aceita
 * o VAD também aceita.
 */
export const PISO = 0.012

/**
 * Quantas vezes o fundo da sala a fala precisa ser.
 *
 * Um número fixo não serve para dois microfones diferentes: o mesmo 0,02 é silêncio
 * num mic com ganho alto e é grito num notebook com ganho baixo. O que separa fala de
 * sala é sempre a MESMA relação — a voz salta acima do que já estava lá — e é isso
 * que se mede aqui, em vez do valor absoluto.
 */
export const MARGEM = 2.5

/**
 * Com que velocidade o fundo da sala é reaprendido — e a assimetria é o coração disto.
 *
 * **Sobe devagar** (~1% por amostra, ou perto de um minuto para convergir) porque a
 * subida não pode confundir voz com sala: uma frase de três segundos são ~15 amostras,
 * que movem o fundo uns 14% do caminho até ela — de menos para o limiar alcançar a
 * própria fala e cortá-la no meio, e o bastante para a TV que ficou ligada acabar
 * aprendida como o barulho que ela é.
 *
 * **Desce rápido** porque a sala que silenciou tem que voltar a ouvir fala baixa em
 * segundos: enquanto o fundo estiver alto demais, quem fala baixo não é ouvido, que é
 * exatamente o defeito que este arquivo existe para não ter.
 *
 * A conta roda em TODA amostra, inclusive nas que são fala. Aprender só no silêncio
 * parece mais limpo e trava: num microfone com ganho alto, o fundo da sala já nasce
 * acima do piso, toda amostra é classificada como fala, e um fundo que só aprende no
 * silêncio nunca sai do zero — o limiar ficaria preso no piso para sempre, e o turno
 * nunca fecharia.
 */
const SOBE = 0.01
const DESCE = 0.3

/**
 * Quantas janelas seguidas acima do limiar são precisas para ABRIR um turno.
 *
 * Difícil de começar, fácil de continuar — a assimetria é de propósito. Uma janela só
 * é o clique do teclado, a porta batendo, a tosse: qualquer um deles abriria o turno e
 * o fecharia 1,2 s depois, e é justamente nesse 1,2 s que a pessoa começa a falar e
 * perde as primeiras sílabas no intervalo em que o microfone fecha para transcrever.
 * Duas janelas são 400 ms de energia contínua, que nenhum estalo tem e qualquer palavra
 * tem de sobra. Uma vez aberto, porém, qualquer pico segura o turno: a frase não pode
 * se cortar sozinha numa vírgula.
 */
const CONFIRMA = 2

/**
 * Silêncio que encerra a frase. Abaixo de ~1 s ele corta quem pensa no meio da
 * oração ("abre o... youtube"); muito acima, cada resposta demora um tempo a mais
 * que o usuário sente como travamento.
 */
export const SILENCIO_MS = 1200

/**
 * Sem nunca ter ouvido nada, a gravação é jogada fora e recomeçada. O `Recorder`
 * acumula as amostras em memória até o `stop`, então a escuta esquecida
 * ligado cresceria sem teto — 30 s de silêncio é lixo de qualquer jeito.
 */
export const OCIOSO_MS = 30_000

export interface TurnoVad {
  /** Instante do último pico acima do limiar, ou `null` se ainda não falou nada. */
  falouEm: number | null
  /** Quando esta gravação abriu — a base do ocioso. */
  desde: number
  /** O fundo da sala, aprendido no silêncio. É metade do limiar de agora. */
  ruido: number
  /** Janelas seguidas acima do limiar, até o turno abrir. Ver [`CONFIRMA`]. */
  acima: number
}

export type DecisaoVad = 'ouvindo' | 'fechar' | 'reciclar'

export function iniciarTurno(agora: number, ruido = 0): TurnoVad {
  // O fundo aprendido ATRAVESSA os turnos: ele é da sala, não da gravação, e zerá-lo
  // a cada frase faria a primeira amostra depois de cada resposta ser medida contra
  // um silêncio que não existe.
  return { falouEm: null, desde: agora, ruido, acima: 0 }
}

/** Acima disto é fala, agora, neste microfone e nesta sala. */
export function limiarDe(turno: TurnoVad): number {
  return Math.max(PISO, turno.ruido * MARGEM)
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
  const passo = level > turno.ruido ? SOBE : DESCE
  const atual = { ...turno, ruido: turno.ruido + (level - turno.ruido) * passo }

  if (level > limiarDe(turno)) {
    const acima = turno.acima + 1
    const abriu = turno.falouEm !== null || acima >= CONFIRMA
    return { decisao: 'ouvindo', turno: { ...atual, acima, falouEm: abriu ? agora : null } }
  }

  atual.acima = 0

  if (atual.falouEm !== null) {
    const decisao = agora - atual.falouEm >= SILENCIO_MS ? 'fechar' : 'ouvindo'
    return { decisao, turno: atual }
  }

  const decisao = agora - atual.desde >= OCIOSO_MS ? 'reciclar' : 'ouvindo'
  return { decisao, turno: atual }
}
