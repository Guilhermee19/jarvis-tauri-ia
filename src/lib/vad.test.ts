/**
 * `node --test` do Node 24, que lê TypeScript sem transpilar — nenhuma dependência
 * de teste entra no projeto por causa deste arquivo. Roda com `npm run test:js`.
 */
import assert from 'node:assert/strict'
import test from 'node:test'

import { avaliarTurno, iniciarTurno, limiarDe, OCIOSO_MS, SILENCIO_MS } from './vad.ts'

/** Encadeia as amostras como o laço real faz, e devolve a última decisão. */
function reproduzir(amostras: Array<{ level: number; em: number }>) {
  let turno = iniciarTurno(0)
  let decisao = 'ouvindo'

  for (const amostra of amostras) {
    const passo = avaliarTurno(turno, amostra.level, amostra.em)
    turno = passo.turno
    decisao = passo.decisao
    if (decisao !== 'ouvindo') break
  }

  return decisao
}

test('silêncio puro não fecha turno — não existe frase para mandar', () => {
  assert.equal(
    reproduzir([
      { level: 0, em: 200 },
      { level: 0.004, em: 5_000 },
    ]),
    'ouvindo',
  )
})

test('fala seguida do silêncio inteiro fecha o turno', () => {
  assert.equal(
    reproduzir([
      { level: 0.4, em: 1_000 },
      { level: 0.4, em: 1_200 },
      { level: 0.01, em: 1_200 + SILENCIO_MS },
    ]),
    'fechar',
  )
})

test('um estalo sozinho não abre turno — clique e tosse não são frase', () => {
  // O pico solto some e o relógio do ocioso continua correndo, que é a prova de que
  // ninguém falou: um turno aberto teria desligado o ocioso.
  assert.equal(
    reproduzir([
      { level: 0.5, em: 200 },
      { level: 0.001, em: 400 },
      { level: 0.001, em: OCIOSO_MS },
    ]),
    'reciclar',
  )
})

test('pausa curta no meio da frase não corta quem está pensando', () => {
  assert.equal(
    reproduzir([
      { level: 0.4, em: 1_000 },
      { level: 0.4, em: 1_200 },
      { level: 0.005, em: 1_400 },
      { level: 0.3, em: 1_600 },
      { level: 0.005, em: 2_000 },
    ]),
    'ouvindo',
  )
})

test('mudo por tempo demais recicla a gravação em vez de fechar turno', () => {
  assert.equal(reproduzir([{ level: 0.001, em: OCIOSO_MS }]), 'reciclar')
})

test('ter falado uma vez desliga o ocioso — o turno espera o silêncio, não o relógio', () => {
  assert.equal(
    reproduzir([
      { level: 0.5, em: OCIOSO_MS - 300 },
      { level: 0.5, em: OCIOSO_MS - 100 },
      { level: 0.001, em: OCIOSO_MS + 100 },
    ]),
    'ouvindo',
  )
})

test('fala baixa fecha o turno — quem fala longe do mic não precisa gritar', () => {
  // 0,014 é fala de verdade para o Whisper (o portão dele é 0,015 no PICO da gravação)
  // e era silêncio para o VAD antigo, que só reconhecia fala a partir de 0,02.
  assert.equal(
    reproduzir([
      { level: 0.014, em: 1_000 },
      { level: 0.014, em: 1_200 },
      { level: 0.002, em: 1_200 + SILENCIO_MS },
    ]),
    'fechar',
  )
})

test('o limiar acompanha o fundo da sala, e o fundo é aprendido', () => {
  // Um chiado constante bem acima do piso: no começo ele passa por fala, e é o fundo
  // subindo que faz o turno se resolver sozinho. Sem isto a gravação ficaria aberta
  // para sempre numa sala barulhenta, porque o pico renova o carimbo a cada amostra.
  let turno = iniciarTurno(0)
  let decisao = 'ouvindo'
  let em = 0

  for (let i = 0; i < 200 && decisao === 'ouvindo'; i += 1) {
    em += 200
    const passo = avaliarTurno(turno, 0.05, em)
    turno = passo.turno
    decisao = passo.decisao
  }

  assert.equal(decisao, 'fechar')
  assert.ok(limiarDe(turno) > 0.05, 'o chiado tem que ter deixado de ser fala')
})

test('o fundo aprendido não engole a fala que vem depois', () => {
  // Mesma sala do teste acima, mas agora alguém fala: o limiar subiu, e voz normal
  // continua muito acima dele.
  const turno = iniciarTurno(0, 0.05)
  const primeira = avaliarTurno(turno, 0.3, 1_000)
  const passo = avaliarTurno(primeira.turno, 0.3, 1_200)

  assert.equal(passo.decisao, 'ouvindo')
  assert.equal(passo.turno.falouEm, 1_200, 'isto era fala')
})
