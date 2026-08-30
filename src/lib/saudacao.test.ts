import assert from 'node:assert/strict'
import test from 'node:test'

// Com a extensão, como o `vad.test.ts`: o `node --test` resolve o módulo sem o
// empacotador, e sem o `.ts` ele não acha o arquivo.
import { comporSaudacao, saudacaoDaHora } from './saudacao.ts'

/** Uma data com a hora cravada, para o teste não depender de quando ele roda. */
function as(hora: number): Date {
  const data = new Date(2026, 0, 15)
  data.setHours(hora, 30, 0, 0)
  return data
}

test('o cumprimento segue a hora, nos cortes do português falado', () => {
  assert.equal(saudacaoDaHora(as(0)), 'Bom dia')
  assert.equal(saudacaoDaHora(as(11)), 'Bom dia')
  // Meio-dia já é tarde.
  assert.equal(saudacaoDaHora(as(12)), 'Boa tarde')
  assert.equal(saudacaoDaHora(as(17)), 'Boa tarde')
  // 18h já é noite.
  assert.equal(saudacaoDaHora(as(18)), 'Boa noite')
  assert.equal(saudacaoDaHora(as(23)), 'Boa noite')
})

test('reconhecido, chama pelo nome', () => {
  assert.equal(
    comporSaudacao('Bom dia', { nome: 'Guilherme', temAlguem: true }),
    'Bom dia, Guilherme.',
  )
})

test('alguém desconhecido vira uma pergunta — é o que faz o cadastro acontecer', () => {
  const frase = comporSaudacao('Boa tarde', { nome: '', temAlguem: true })

  assert.match(frase, /^Boa tarde\./)
  assert.match(frase, /quem é você\?$/)
})

test('sem ninguém na frente, saúda e cala', () => {
  // Perguntar "quem é você?" para uma cadeira vazia faz o assistente parecer quebrado.
  assert.equal(comporSaudacao('Boa noite', { nome: '', temAlguem: false }), 'Boa noite.')
})

test('câmera falhou é o mesmo que cadeira vazia: saúda sem nome e sem pergunta', () => {
  // `null` é o que o hook passa quando o reconhecimento nem chegou a rodar (modelos não
  // instalados, câmera ocupada). Não há a quem perguntar.
  assert.equal(comporSaudacao('Bom dia', null), 'Bom dia.')
})

test('nome vazio nunca vira "Bom dia, ." — o espaço em branco tem que cair no genérico', () => {
  assert.equal(comporSaudacao('Bom dia', { nome: '', temAlguem: false }), 'Bom dia.')
})
