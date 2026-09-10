/**
 * `node --test` do Node 24, que lê TypeScript sem transpilar — nenhuma dependência
 * de teste entra no projeto por causa deste arquivo. Roda com `npm run test:js`.
 *
 * O que está sob teste é o ÚNICO portão entre o microfone aberto e o roteador que abre
 * programas: com a escuta ligada, tudo que é falado na sala passa por aqui.
 */
import assert from 'node:assert/strict'
import test from 'node:test'

import { comandoEnderecado } from './dictation.ts'

const NOME = 'Jarvis'

test('o nome na frente vira comando, sem o nome', () => {
  assert.equal(comandoEnderecado('Jarvis, abre o youtube', NOME), 'abre o youtube')
})

test('um vocativo antes do nome ainda é chamado', () => {
  assert.equal(comandoEnderecado('ei Jarvis, que horas são?', NOME), 'que horas são?')
})

test('o Whisper acentuando o nome não perde o comando', () => {
  assert.equal(comandoEnderecado('Járvis abre o spotify', NOME), 'abre o spotify')
})

test('falar DELE não é falar COM ele — o resto da sala é descartado', () => {
  assert.equal(comandoEnderecado('ontem eu falei com o Jarvis sobre isso', NOME), null)
})

test('chamar não é mandar: o nome sozinho não vai para o modelo', () => {
  assert.equal(comandoEnderecado('Jarvis?', NOME), null)
})

test('o comando sai com acento e pontuação — é o modelo que vai lê-lo', () => {
  assert.equal(comandoEnderecado('Jarvis: põe música, por favor!', NOME), 'põe música, por favor!')
})

test('o nome segue a configuração — outro assistente, outro gatilho', () => {
  assert.equal(comandoEnderecado('Ultron, desliga a luz', 'Ultron'), 'desliga a luz')
  assert.equal(comandoEnderecado('Jarvis, desliga a luz', 'Ultron'), null)
})

test('silêncio transcrito em nada não acorda ninguém', () => {
  assert.equal(comandoEnderecado('', NOME), null)
  assert.equal(comandoEnderecado('   ', NOME), null)
})

test('o nome mal ouvido ainda é o nome — o ditado erra letra, quem chamou não errou', () => {
  assert.equal(comandoEnderecado('Jarves, abre o youtube', NOME), 'abre o youtube')
  assert.equal(comandoEnderecado('Jarvi, que horas são?', NOME), 'que horas são?')
  assert.equal(comandoEnderecado('Jarvys liga a luz', NOME), 'liga a luz')
})

test('o nome partido em duas palavras pelo ditado continua sendo o nome', () => {
  assert.equal(comandoEnderecado('Jar vis, liga a luz da sala', NOME), 'liga a luz da sala')
  assert.equal(comandoEnderecado('ei Já vis, toca música', NOME), 'toca música')
})

test('a folga é de uma letra, e não de qualquer nome parecido', () => {
  assert.equal(comandoEnderecado('Marcos, abre o youtube', NOME), null)
  assert.equal(comandoEnderecado('Sandra, desliga tudo', NOME), null)
  assert.equal(comandoEnderecado('Travis, abre o youtube', NOME), null)
})

test('nome curto não ganha folga nenhuma — uma letra ali já é outro nome', () => {
  assert.equal(comandoEnderecado('Ana, liga a luz', 'Ana'), 'liga a luz')
  assert.equal(comandoEnderecado('Ane, liga a luz', 'Ana'), null)
})
