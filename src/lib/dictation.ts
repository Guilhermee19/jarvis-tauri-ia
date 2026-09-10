/**
 * Quando uma frase ouvida é um COMANDO endereçado ao assistente.
 *
 * É o filtro da escuta contínua, e o único: com o microfone aberto o tempo todo, TUDO
 * que é dito na sala chega transcrito até aqui — conversa entre duas pessoas, a TV, o
 * telefone. Do outro lado tem um roteador que abre programas, então deixar tudo passar
 * transformaria uma tarde de papo numa fila de comandos.
 *
 * O nome é o gatilho: dizer "Jarvis, abre o youtube" é uma declaração explícita de que a
 * frase é para ele, e é o que as pessoas já fazem naturalmente ao chamar alguém numa
 * sala. O que não é endereçado morre aqui, calado — falar perto do microfone não é
 * falar com ele.
 *
 * ## Por que o nome não é comparado letra a letra
 *
 * Quem chama não digita. O Whisper devolve "Jarves", "Jarvi", "Jarvys" e às vezes o
 * nome PARTIDO em duas palavras ("jar vis"), e exigir a string exata fazia todas essas
 * frases morrerem caladas — o mesmo silêncio de quem não falou nada, que é o pior jeito
 * de errar, porque não dá nem para saber que se errou. A folga é de UMA letra: o
 * suficiente para o ditado tropeçar, pouco o bastante para "Marcos" e "Sandra" não
 * abrirem programa na sua máquina.
 */

/** Vocativos que costumam vir antes do nome. "ô Jarvis" é comum em português. */
const CHAMAMENTOS = ['ei', 'oi', 'ola', 'alo', 'hey', 'o', 'ah', 'e', 'opa']

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
 * Quantas letras de engano cabem num nome deste tamanho.
 *
 * Cresce com o tamanho porque o estrago do engano é que muda: em nome curto uma letra
 * já é outra palavra inteira ("Ana" e "Ane"), e em nome longo uma ou duas quase nunca
 * são. Nome de até quatro letras, por isso, continua exigindo igualdade.
 */
function folga(nome: string): number {
  if (nome.length <= 4) return 0
  return nome.length <= 7 ? 1 : 2
}

/** Se uma palavra do ditado é o nome do assistente mal ouvido. */
function ehONome(dita: string, nome: string): boolean {
  const tolerancia = folga(nome)
  if (tolerancia === 0) return dita === nome

  // Diferença de tamanho já é distância mínima, e sai mais barato que a matriz.
  if (Math.abs(dita.length - nome.length) > tolerancia) return false
  return distancia(dita, nome) <= tolerancia
}

/**
 * Quantas letras é preciso trocar, tirar ou pôr para chegar de uma palavra na outra
 * (Levenshtein, uma linha por vez — isto roda sobre palavras, não sobre textos).
 */
function distancia(a: string, b: string): number {
  const linha = Array.from({ length: b.length + 1 }, (_, i) => i)

  for (let i = 0; i < a.length; i += 1) {
    let diagonal = linha[0]
    linha[0] = i + 1

    for (let j = 0; j < b.length; j += 1) {
      const trocando = diagonal + (a[i] === b[j] ? 0 : 1)
      diagonal = linha[j + 1]
      linha[j + 1] = Math.min(trocando, linha[j] + 1, diagonal + 1)
    }
  }

  return linha[b.length]
}

/**
 * Depois de quantas palavras o comando começa, ou `-1` se o nome não foi dito.
 *
 * O nome pode vir depois de um vocativo ("ei Jarvis"), mas só de UM: exigir que ele
 * esteja no começo é o que impede "falei com o Jarvis ontem" de virar comando.
 */
function ondeAcabaONome(ditas: string[], nome: string): number {
  const inicios = ditas.length > 1 && CHAMAMENTOS.includes(ditas[0]) ? [0, 1] : [0]

  for (const inicio of inicios) {
    if (ehONome(ditas[inicio], nome)) return inicio + 1

    // O ditado às vezes PARTE o nome em duas ("jar vis", "já vis"), e as duas metades
    // sozinhas não se parecem com nada. Coladas, voltam a ser o nome.
    if (inicio + 1 < ditas.length && ehONome(ditas[inicio] + ditas[inicio + 1], nome)) {
      return inicio + 2
    }
  }

  return -1
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

  const quantas = ondeAcabaONome(ditas, nome)
  if (quantas === -1) return null

  // Recortar do TEXTO ORIGINAL, não das palavras normalizadas: o comando vai para o
  // modelo, e ele precisa dos acentos e da pontuação que o Whisper produziu.
  const comando = recortarDepoisDoNome(transcrito, quantas)
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
