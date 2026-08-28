/** Espelho de `src-tauri/src/core/casa.rs`. Serde serializa em camelCase. */

/**
 * Um aparelho anunciado na rede local.
 *
 * Não tem `nome`: o anúncio que os aparelhos fazem na rede não carrega o nome que você
 * deu no app. Esse vem da nuvem da Tuya junto com a chave de controle, na próxima fase.
 */
export interface Aparelho {
  id: string
  ip: string
  /** "3.1", "3.3", "3.4", "3.5"… decide o que dá para usar para controlar. */
  versao: string
  produto: string | null
  /** `false` costuma ser aparelho novo, ainda esperando configuração no app. */
  ativo: boolean
  /** `false` quando o anúncio veio em texto puro (3.1) ou quando não abriu. */
  decifrado: boolean
  /**
   * Se dá para MANDAR COMANDO nele pelo caminho que o Jarvis fala.
   *
   * `false` no 3.5: o anúncio dele é lido (id, modelo, versão), mas o controle usa outro
   * quadro. O aparelho aparece na lista de qualquer jeito.
   */
  suportado: boolean
  /**
   * O nome que você deu no app ("Luz Cozinha"), vindo do chaveiro.
   *
   * `null` = a nuvem ainda não foi consultada. O anúncio da rede nunca traz nome.
   */
  nome: string | null
  /**
   * Se a `local_key` dele já foi importada.
   *
   * Independente de `suportado`: sem chave não há comando por mais conhecido que o
   * protocolo seja, e sobra chave para aparelho que ainda não sabemos comandar.
   */
  temChave: boolean
  /**
   * Se a rede o anunciou **nesta** varredura.
   *
   * A lista mistura de propósito quem está aqui agora com quem já esteve: um aparelho
   * desligado da tomada não deve sumir da tela, senão o app parece ter esquecido dele.
   * Mas os dois não podem parecer a mesma coisa — o que está fora do ar não obedece.
   */
  presente: boolean
  /** Quando a rede o anunciou pela última vez, em ms. `0` = nunca. */
  vistoEm: number
  /**
   * A categoria da Tuya: "dj" (lâmpada), "cz" (tomada), "wg2" (gateway)…
   *
   * Vazia até a importação acontecer — o anúncio da rede não diz que tipo de coisa ele
   * é. Vira o ícone do cartão.
   */
  categoria: string
  /**
   * Se este TIPO de aparelho tem um liga-desliga que faça sentido oferecer.
   *
   * Independente de `suportado` e `temChave`: uma central ZigBee responde a tudo e não
   * tem o que ligar. Sem essa separação o botão apareceria nela e alternaria um data
   * point booleano que ninguém sabe o que faz.
   */
  comutavel: boolean
  /**
   * Tirado da lista principal por escolha sua.
   *
   * **Ocultar é sobre a tela, não sobre o aparelho.** Ele continua sendo varrido,
   * continua com a chave guardada e continua obedecendo por voz — o que muda é ele não
   * disputar espaço com o que você usa todo dia.
   */
  oculto: boolean
  /**
   * O emissor de infravermelho que emite por este controle, quando ele é um.
   *
   * Vazio em aparelho de rede. Preenchido, quer dizer que este cartão é uma TV ou um
   * ar-condicionado: **sem IP, sem protocolo e sem estado**, comandado por teclas em vez
   * de botão. Eles nunca aparecem na varredura porque não têm Wi-Fi — existem como uma
   * lista de códigos dentro do emissor.
   */
  emissor: string
  /**
   * Subaparelho ZigBee: ele **não fala na rede**, quem fala é o gateway.
   *
   * Sem isto a tela o trataria como um aparelho de Wi-Fi que sumiu — "fora do ar",
   * "visto nunca" — quando ele nunca esteve na rede e nem deveria estar.
   */
  subaparelho: boolean
}

/** Uma tecla de controle remoto. Espelha `Tecla` de `core/casa/nuvem.rs`. */
export interface Tecla {
  /** O código que vai no comando ("Power", "Channel+"). */
  key: string
  keyId: number
  /** O rótulo legível ("Channel Up"). Costuma ser melhor que o código. */
  keyName: string
}

/** Um controle e o que dá para apertar nele. */
export interface Controle {
  /**
   * A categoria do controle na biblioteca da Tuya.
   *
   * Anda junto das teclas porque o envio **exige as duas**: sem ela a Tuya recusa com um
   * `categoryId` seco, que não diz que ela faltou.
   */
  categoria: number
  teclas: Tecla[]
}

/**
 * O que uma importação da nuvem trouxe. Espelha `Importado` de
 * `src-tauri/src/commands/casa.rs`.
 *
 * Sem a chave de controle de propósito: ela fica no Rust. A UI não tem o que fazer com
 * ela, e trazê-la só aumentaria o número de lugares por onde um segredo pode vazar.
 */
export interface Importado {
  id: string
  nome: string
  temChave: boolean
}

/**
 * O que o aparelho confirmou depois de um comando. Espelha `Estado` de
 * `src-tauri/src/core/casa/controle.rs`.
 */
export interface EstadoAparelho {
  ligado: boolean
  /**
   * Qual data point acabou sendo o liga-desliga deste modelo.
   *
   * Aparelho Tuya não tem comando "ligar" — tem DPs numerados, e qual deles é o
   * interruptor muda por modelo. Quando um aparelho não obedece, saber se ele foi
   * comandado pelo `1` ou pelo `20` é a primeira coisa que se quer olhar.
   */
  interruptor: string
}

export interface Varredura {
  aparelhos: Aparelho[]
  /**
   * Pacotes que chegaram e não viraram aparelho.
   *
   * Existe para separar dois silêncios que dão a mesma tela vazia: **ninguém falou**
   * (rede errada, firewall) e **falaram e não entendi** (formato desconhecido). As
   * soluções são opostas, e sem esse número as duas parecem defeito de rede.
   */
  ignorados: number
}

/**
 * O estado de uma lâmpada, já traduzido do catálogo de data points da Tuya. Espelha
 * `Luz` de `src-tauri/src/core/casa/controle.rs`.
 */
export interface Luz {
  /** "white", "colour", "scene" ou "music". */
  modo: string
  /** 10 a 1000. */
  brilho: number
  /** 0 (branco quente) a 1000 (branco frio). */
  temperatura: number
  /** 0 a 360. */
  matiz: number
  /** 0 a 1000. */
  saturacao: number
  /**
   * Quais ajustes ESTE aparelho aceita.
   *
   * Sai dos data points que ele expõe, e não da categoria: a categoria diz o que a nuvem
   * acha que ele é, os DPs dizem o que ele realmente faz. Uma lâmpada só de branco não
   * tem o DP da cor, e um seletor que não faz nada é pior que a ausência dele.
   */
  temCor: boolean
  temBrilho: boolean
  temBranco: boolean
}

/**
 * Um liga-desliga do aparelho.
 *
 * Plural porque **um aparelho pode ter vários**: uma tomada dupla responde `1` e `2`, e
 * mostrar só o primeiro deixaria metade dela sem botão.
 */
export interface Chave {
  dp: string
  rotulo: string
  ligado: boolean
}

/**
 * Uma medida que o aparelho reporta e que **não se comanda** — o estado de uma porta, a
 * bateria de um sensor.
 *
 * Vem já em português do Rust: o booleano de um sensor é uma frase, e o significado dela
 * muda com o aparelho. `true` num sensor de porta quer dizer "aberta".
 */
export interface Leitura {
  rotulo: string
  valor: string
}

/** O retrato completo de um aparelho. Espelha `Detalhe` do mesmo módulo. */
export interface DetalheAparelho {
  ligado: boolean
  /** Qual data point acabou sendo o liga-desliga deste modelo. */
  interruptor: string
  /** Todos os liga-desliga, não só o principal. */
  chaves: Chave[]
  /** O que o aparelho mede e não se comanda. */
  leituras: Leitura[]
  /** `null` quando os data points não revelam uma lâmpada. */
  luz: Luz | null
  /**
   * Os data points crus, do jeito que o aparelho respondeu.
   *
   * É o que permite descobrir um aparelho que faz algo que este app ainda não modela —
   * e é a primeira coisa a olhar quando um comando não pega.
   */
  dps: Record<string, unknown>
}

/** O que mudar numa lâmpada. Campo ausente fica como está. */
export interface AjusteLuz {
  ligado?: boolean
  brilho?: number
  temperatura?: number
  /** Matiz e saturação andam juntos: a Tuya guarda os dois no mesmo data point. */
  matiz?: number
  saturacao?: number
}
