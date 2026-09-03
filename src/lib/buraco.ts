/**
 * Onde o webview do navegador cabe, depois de descontar quem está por cima dele.
 *
 * **A página do navegador não é HTML.** Cada aba é um webview NATIVO, empilhado acima de
 * todo o desenho da janela — nenhum `z-index` alcança ele. Por isso a regra antiga era
 * simples e severa: se outra janelinha viesse à frente, o webview sumia inteiro, senão
 * ele cobriria a janelinha que estava por cima.
 *
 * Severa demais. Duas janelinhas lado a lado não se cobrem em pixel nenhum, e ainda assim
 * a página sumia — que é o incômodo que este arquivo existe para resolver. O que importa
 * não é quem está na FRENTE, é quem está **em cima do mesmo lugar**.
 *
 * A conta é geométrica e roda a cada movimento de qualquer janelinha, então ela é
 * deliberadamente burra: nada de subtração de polígonos, só o maior retângulo que sobra.
 */
export interface Caixa {
  x: number
  y: number
  largura: number
  altura: number
}

/**
 * Menor que isto não é página, é uma tira. Melhor esconder e dizer o porquê do que
 * mostrar 20 pixels de site e deixar a pessoa achar que quebrou.
 */
const MINIMO = { largura: 180, altura: 120 }

/**
 * O que sobra da `area` depois de tirar cada obstáculo, ou `null` se não sobrou página.
 *
 * Os obstáculos são aplicados em sequência, então o resultado depende da ordem quando há
 * mais de um — e tudo bem: qualquer retângulo livre serve, e o caso de duas janelinhas
 * cobrindo o navegador ao mesmo tempo é raro o suficiente para não merecer a busca pelo
 * ótimo.
 */
export function descontar(area: Caixa, obstaculos: Caixa[]): Caixa | null {
  let livre: Caixa = area

  for (const obstaculo of obstaculos) {
    livre = cortar(livre, obstaculo)
  }

  const cabe = livre.largura >= MINIMO.largura && livre.altura >= MINIMO.altura
  return cabe ? livre : null
}

/**
 * A maior das quatro fatias que sobram quando o obstáculo entra na área.
 *
 * Sem interseção, a área volta intacta — que é o caso comum e o motivo de tudo isto: duas
 * janelinhas abertas raramente se sobrepõem.
 */
function cortar(area: Caixa, obstaculo: Caixa): Caixa {
  const esquerda = Math.max(area.x, obstaculo.x)
  const direita = Math.min(area.x + area.largura, obstaculo.x + obstaculo.largura)
  const topo = Math.max(area.y, obstaculo.y)
  const base = Math.min(area.y + area.altura, obstaculo.y + obstaculo.altura)

  if (direita <= esquerda || base <= topo) return area

  const fatias: Caixa[] = [
    { ...area, largura: esquerda - area.x },
    { ...area, x: direita, largura: area.x + area.largura - direita },
    { ...area, altura: topo - area.y },
    { ...area, y: base, altura: area.y + area.altura - base },
  ]

  return fatias.reduce((maior, fatia) => (espaco(fatia) > espaco(maior) ? fatia : maior))
}

function espaco(caixa: Caixa): number {
  return Math.max(0, caixa.largura) * Math.max(0, caixa.altura)
}
