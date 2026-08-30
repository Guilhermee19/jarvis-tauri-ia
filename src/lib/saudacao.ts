/**
 * A frase de abertura do Jarvis: "Bom dia, Guilherme."
 *
 * Só as duas decisões PURAS moram aqui — qual cumprimento a hora pede, e o que dizer a
 * partir de quem foi (ou não) reconhecido. A câmera, a fala e o momento de disparar ficam
 * no `useSaudacao`, que é o que permite testar as frases sem webcam nenhuma.
 */

/** O que o reconhecimento devolveu, do pouco que a frase precisa saber. */
export interface QuemFoiVisto {
  nome: string
  temAlguem: boolean
}

/**
 * "Bom dia", "boa tarde" ou "boa noite", pela hora do relógio.
 *
 * Os cortes são os do português falado: a tarde começa ao meio-dia e a noite às 18h.
 */
export function saudacaoDaHora(agora: Date): string {
  const hora = agora.getHours()

  if (hora < 12) return 'Bom dia'
  if (hora < 18) return 'Boa tarde'
  return 'Boa noite'
}

/**
 * A frase inteira, a partir de quem foi (ou não) reconhecido.
 *
 * Três desfechos, e a diferença entre os dois últimos é o ponto do desenho:
 *
 * - **Reconhecido**: chama pelo nome. É o que a feature existe para fazer.
 * - **Alguém desconhecido**: saúda e PERGUNTA. Sem isso o cadastro nunca acontece — a
 *   pessoa teria que descobrir sozinha que existe um jeito de se apresentar.
 * - **Ninguém na frente**: saúda e cala. Perguntar "quem é você?" para uma cadeira vazia
 *   é o tipo de coisa que faz um assistente parecer quebrado. Vale também quando a
 *   câmera falhou, porque aí também não há a quem perguntar.
 */
export function comporSaudacao(hora: string, quem: QuemFoiVisto | null): string {
  if (quem?.nome) return `${hora}, ${quem.nome}.`
  if (quem?.temAlguem) return `${hora}. Ainda não conheço seu rosto — quem é você?`

  return `${hora}.`
}
