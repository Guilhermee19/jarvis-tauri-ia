/**
 * `node --test` do Node 24, que lê TypeScript sem transpilar — nenhuma dependência
 * de teste entra no projeto por causa deste arquivo. Roda com `npm run test:js`.
 */
import assert from 'node:assert/strict'
import test from 'node:test'

import { avaliarTurno, iniciarTurno, OCIOSO_MS, SILENCIO_MS } from './vad.ts'

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
      { level: 0.01, em: 1_000 + SILENCIO_MS },
    ]),
    'fechar',
  )
})

test('pausa curta no meio da frase não corta quem está pensando', () => {
  assert.equal(
    reproduzir([
      { level: 0.4, em: 1_000 },
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
      { level: 0.5, em: OCIOSO_MS - 100 },
      { level: 0.001, em: OCIOSO_MS + 100 },
    ]),
    'ouvindo',
  )
})
