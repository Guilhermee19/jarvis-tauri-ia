/**
 * Quando uma frase ditada é um COMANDO endereçado ao assistente.
 *
 * O ditado enche o campo e espera o "Enviar" de propósito — o Whisper erra, e do
 * outro lado tem um roteador que abre programas. Mas exigir o clique também mata o
 * uso sem as mãos, que é o ponto de falar em vez de digitar.
 *
 * O nome resolve os dois: dizer "Jarvis, abre o youtube" é uma declaração explícita
 * de que a frase é para ele, e é o que as pessoas já fazem naturalmente. Sem o nome,
 * o texto continua indo para o campo — falar perto do microfone não vira comando.
 *
 * É a mesma assimetria do resto do app: ação normal passa, ação sem volta pede um
 * gesto a mais.
 */

/** Vocativos que costumam vir antes do nome. "ô Jarvis" é comum em português. */
const CHAMAMENTOS = ['ei', 'oi', 'ola', 'hey', 'o']

/**
 * Caixa baixa e sem acento, para "Járvis" (que o Whisper produz) casar com "Jarvis".
 * A forma decomposta separa a letra do acento, e aí o acento é removido sozinho.
 */
function normalizar(texto: string): string {
  return texto
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
}

/** Só letras e dígitos contam como palavra; o resto é pontuação a ignorar. */
function palavras(texto: string): string[] {
  return normalizar(texto)
    .split(/[^\p{L}\p{N}]+/u)
    .filter(Boolean)
}

/**
 * Devolve o comando SEM o nome, ou `null` se a frase não foi endereçada.
 *
 * `null` também para o nome sozinho ("Jarvis?"): chamar não é mandar, e mandar uma
 * string vazia para o roteador só gastaria uma volta no modelo.
 */
export function comandoEnderecado(transcrito: string, nomeDoAssistente: string): string | null {
  const nome = normalizar(nomeDoAssistente).trim()
  if (!nome) return null

  const ditas = palavras(transcrito)
  if (ditas.length === 0) return null

  // O nome pode vir depois de um vocativo ("ei Jarvis"), mas só de UM: exigir que ele
  // apareça no começo é o que impede "falei com o Jarvis ontem" de virar comando.
  let indiceDoNome = -1
  if (ditas[0] === nome) {
    indiceDoNome = 0
  } else if (ditas.length > 1 && CHAMAMENTOS.includes(ditas[0]) && ditas[1] === nome) {
    indiceDoNome = 1
  }

  if (indiceDoNome === -1) return null

  // Recortar do TEXTO ORIGINAL, não das palavras normalizadas: o comando vai para o
  // modelo, e ele precisa dos acentos e da pontuação que o Whisper produziu.
  const comando = recortarDepoisDoNome(transcrito, indiceDoNome + 1)
  return comando || null
}

/**
 * Pula as `quantas` primeiras palavras do texto original e devolve o resto.
 *
 * Anda pelo texto de verdade em vez de refazer o split porque a normalização perde
 * as posições — e é justamente o texto original que precisa sobreviver.
 */
function recortarDepoisDoNome(texto: string, quantas: number): string {
  const separador = /[^\p{L}\p{N}]+/u
  let posicao = 0
  let vistas = 0

  while (vistas < quantas && posicao < texto.length) {
    // Pula a pontuação e o espaço antes da próxima palavra.
    while (posicao < texto.length && separador.test(texto[posicao])) posicao += 1
    // Consome a palavra.
    while (posicao < texto.length && !separador.test(texto[posicao])) posicao += 1
    vistas += 1
  }

  // O que sobrou ainda começa na pontuação que separava ("Jarvis, abre" → ", abre").
  return texto
    .slice(posicao)
    .replace(/^[^\p{L}\p{N}]+/u, '')
    .trim()
}
