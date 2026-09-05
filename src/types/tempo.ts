/**
 * Espelho de `src-tauri/src/core/tempo.rs`.
 *
 * Os campos são de uma palavra só, então não há `rename_all` do lado do Rust e os nomes
 * chegam iguais. Se um lado mudar, o outro precisa mudar junto.
 */

/** Um dia da previsão. */
export interface DiaDeTempo {
  /** `YYYY-MM-DD`, no fuso do LUGAR — é o `timezone=auto` da consulta que garante isso. */
  data: string
  minima: number
  maxima: number
  /** Código WMO do céu. Vira desenho e rótulo no `ceuDoCodigo`. */
  ceu: number
  /** Probabilidade máxima de precipitação no dia, em %. */
  chuva: number
}

export interface Previsao {
  temperatura: number
  umidade: number
  ceu: number
  dias: DiaDeTempo[]
}

/** As famílias de céu que o card desenha. */
export type CeuId = 'limpo' | 'poucas-nuvens' | 'nublado' | 'nevoa' | 'chuva' | 'trovoada' | 'neve'

/**
 * Código WMO → o que desenhar e como chamar.
 *
 * **O agrupamento é mais grosso que o do Rust, e de propósito.** O `tempo.rs::descricao`
 * separa "chuva fraca", "moderada" e "forte" porque isso é dito em voz alta e a diferença
 * cabe numa frase; aqui cada família precisa de um DESENHO, e três gotas diferentes não
 * comunicam nada de relance. O rótulo abaixo repete as palavras de lá para a tela e a fala
 * não se contradizerem na mesma tela.
 */
export function ceuDoCodigo(codigo: number): { id: CeuId; rotulo: string } {
  if (codigo === 0) return { id: 'limpo', rotulo: 'Céu limpo' }
  if (codigo === 1) return { id: 'limpo', rotulo: 'Quase limpo' }
  if (codigo === 2) return { id: 'poucas-nuvens', rotulo: 'Parcialmente nublado' }
  if (codigo === 3) return { id: 'nublado', rotulo: 'Nublado' }
  if (codigo === 45 || codigo === 48) return { id: 'nevoa', rotulo: 'Névoa' }
  if (codigo === 51 || codigo === 53 || codigo === 55) return { id: 'chuva', rotulo: 'Garoa' }
  if (codigo === 56 || codigo === 57) return { id: 'chuva', rotulo: 'Garoa congelante' }
  if (codigo === 61 || codigo === 80) return { id: 'chuva', rotulo: 'Chuva fraca' }
  if (codigo === 63 || codigo === 81) return { id: 'chuva', rotulo: 'Chuva moderada' }
  if (codigo === 65 || codigo === 82) return { id: 'chuva', rotulo: 'Chuva forte' }
  if (codigo === 66 || codigo === 67) return { id: 'chuva', rotulo: 'Chuva congelante' }
  if (codigo === 71 || codigo === 73 || codigo === 75 || codigo === 77) {
    return { id: 'neve', rotulo: 'Neve' }
  }
  if (codigo === 85 || codigo === 86) return { id: 'neve', rotulo: 'Pancadas de neve' }
  if (codigo === 95) return { id: 'trovoada', rotulo: 'Trovoada' }
  if (codigo === 96 || codigo === 99) return { id: 'trovoada', rotulo: 'Trovoada com granizo' }

  // O `_` do Rust é "sem detalhe do céu", e aqui ele precisa de um desenho: nublado é o
  // mais neutro dos seis — não promete sol que pode não vir nem chuva que não foi dita.
  return { id: 'nublado', rotulo: 'Sem detalhe' }
}
