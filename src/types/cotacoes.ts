/**
 * Espelho de `src-tauri/src/core/cotacoes.rs`, que serializa em camelCase.
 *
 * Os quatro pares vêm sempre na mesma ordem (dólar, euro, bitcoin, ethereum) — quem
 * ordena é o Rust, e de propósito: ordem de card que dança entre uma abertura e outra
 * parece bug.
 */
export interface Cotacao {
  /** `USD`, `EUR`, `BTC`, `ETH`. Chave estável para escolher rótulo e ordem. */
  codigo: string
  /** "Dólar Americano/Real Brasileiro", como a fonte manda. */
  nome: string
  /** Preço de compra (`bid`), em reais. */
  valor: number
  /** Variação do dia, em porcentagem. **Com sinal** — é ela que dá a cor. */
  variacao: number
  minima: number
  maxima: number
  /** `YYYY-MM-DD HH:MM:SS`. Cotação sem hora é número sem idade. */
  quando: string
}

/** O rótulo curto do card. "Dólar Americano/Real Brasileiro" não cabe e ninguém fala assim. */
export const NOME_DA_MOEDA: Record<string, string> = {
  USD: 'Dólar',
  EUR: 'Euro',
  BTC: 'Bitcoin',
  ETH: 'Ethereum',
}
