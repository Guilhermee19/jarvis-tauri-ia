import assert from 'node:assert/strict'
import test from 'node:test'

import { descontar, type Caixa } from './buraco.ts'

const NAVEGADOR: Caixa = { x: 600, y: 0, largura: 800, altura: 600 }

/**
 * O caso que motivou o arquivo: conversa à esquerda, navegador à direita, nenhum pixel
 * em comum. A regra antiga sumia com a página assim mesmo, porque olhava a ORDEM das
 * janelinhas em vez do lugar delas.
 */
test('janelinha ao lado não esconde a página', () => {
  const conversa: Caixa = { x: 0, y: 40, largura: 560, altura: 620 }

  assert.deepEqual(descontar(NAVEGADOR, [conversa]), NAVEGADOR)
})

test('janelinha por cima de uma borda encolhe a página, não a apaga', () => {
  // Cobre os 300 px da esquerda do navegador.
  const conversa: Caixa = { x: 400, y: 0, largura: 500, altura: 600 }
  const livre = descontar(NAVEGADOR, [conversa])

  assert.deepEqual(livre, { x: 900, y: 0, largura: 500, altura: 600 })
})

test('janelinha cobrindo tudo esconde mesmo — não há onde pôr a página', () => {
  const enorme: Caixa = { x: 0, y: 0, largura: 1920, altura: 1080 }

  assert.equal(descontar(NAVEGADOR, [enorme]), null)
})

/** Sobra uma tira de 40 px: mostrar isso é pior que esconder e explicar. */
test('sobra pequena demais conta como escondida', () => {
  const quase: Caixa = { x: 600, y: 0, largura: 760, altura: 600 }

  assert.equal(descontar(NAVEGADOR, [quase]), null)
})

test('duas janelinhas em cantos opostos ainda deixam página', () => {
  const canto: Caixa = { x: 600, y: 0, largura: 200, altura: 600 }
  const outra: Caixa = { x: 1300, y: 0, largura: 100, altura: 600 }
  const livre = descontar(NAVEGADOR, [canto, outra])

  assert.ok(livre)
  assert.ok(livre.x >= 800 && livre.x + livre.largura <= 1300, `sobrou ${JSON.stringify(livre)}`)
})

/** Sem ninguém por cima, o buraco é o buraco. */
test('sem obstáculo, a área passa inteira', () => {
  assert.deepEqual(descontar(NAVEGADOR, []), NAVEGADOR)
})
