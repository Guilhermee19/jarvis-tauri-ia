/** Espelha `AppSettings` de `src-tauri/src/config/mod.rs` (serde em camelCase). */

/**
 * O tema: **a cor do app, a voz e o jeito de falar**, num campo só.
 *
 * São três coisas que sempre andam juntas — um Ultron com a voz e o azul do Jarvis não
 * seria o Ultron. O NOME fica de fora de propósito: ele é o gatilho de voz e continua
 * texto livre; trocar de tema sugere o nome, não o impõe.
 */
export type Persona = 'jarvis' | 'ultron'

/** O nome que cada tema sugere. Espelha `Persona::nome()` no Rust. */
export const NOME_DA_PERSONA: Record<Persona, string> = {
  jarvis: 'Jarvis',
  ultron: 'Ultron',
}

export interface AppSettings {
  /** Guardada em disco sem validação nesta versão. */
  anthropicApiKey: string
  /** O nome, que é também o gatilho de voz. Texto livre. */
  assistantName: string
  /** O tema: cor, voz e tom. Ver [`Persona`]. */
  persona: Persona
  /**
   * Clipe de voz clonada, um por persona — o Jarvis e o Ultron não podem soar igual, e
   * reconfigurar a cada troca faria a troca não valer a pena.
   *
   * O valor é o nome do arquivo guardado no servidor de voz local. Vazio = o primeiro
   * clipe cadastrado, e **vazio também é o que faz o Jarvis não falar**: sem clipe não há
   * voz para clonar, e o app fica quieto em vez de errar.
   */
  ttsVoiceJarvis: string
  ttsVoiceUltron: string
  /** Onde o Ollama escuta. Aponta para outra máquina se o Ollama não roda aqui. */
  ollamaUrl: string
  /** Modelo que interpreta, conversa E enxerga. Vazio DESLIGA o intérprete. */
  ollamaModel: string
  /** Pasta da memória em markdown. Vazio = `memoria/` no projeto. */
  memoriaPath: string
  /** Chave do Brave Search. Vazio = Wikipedia (sem chave, mas só enciclopédia). */
  braveApiKey: string
  /** Credenciais do Spotify. Vazias = "toque X" abre a busca em vez de tocar. */
  spotifyClientId: string
  spotifyClientSecret: string
  /**
   * Credenciais do projeto Cloud da Tuya (`iot.tuya.com`). Vazias deixam a Casa em modo
   * só-leitura: a varredura acha os aparelhos, mas sem a chave de cada um não há comando.
   *
   * Servem uma vez, no botão de importar. A chave que sai de lá é do APARELHO e continua
   * valendo depois que o projeto trial expira.
   */
  tuyaClientId: string
  tuyaClientSecret: string
  /**
   * O *data center* do projeto: `us`, `eu`, `cn` ou `in`.
   *
   * Errado, a Tuya responde sucesso com uma lista VAZIA em vez de recusar — não há nada
   * na resposta que aponte para cá. Conta brasileira do Smart Life quase sempre é `us`.
   */
  tuyaRegiao: string
  /**
   * Resolução pedida à webcam. `0` em qualquer um dos dois = automático (o formato
   * mais perto de 640×480). É um pedido: a câmera decide o que consegue entregar.
   */
  webcamWidth: number
  webcamHeight: number
  /** Espelhar a imagem na tela (visão de selfie). Só exibição — não muda os bytes. */
  webcamMirror: boolean
  /** Nome do dispositivo de entrada de áudio. Vazio = padrão do sistema. */
  micDeviceName: string
  /**
   * Mostrar o bloco de log em TODA mensagem, inclusive conversa que não mexeu em nada.
   *
   * Desligado, o log só aparece quando houve ação ou mudança de memória. Ligado, dá para
   * ver o VERBO que o roteador escolheu mesmo quando ele não executou nada — que é o que
   * separa "ele entendeu como conversa" de "ele executou a coisa errada".
   */
  logDetalhado: boolean
}

/**
 * O clipe de voz da persona ATIVA — o gêmeo de `AppSettings::voz()` no Rust.
 *
 * Existe porque **três lugares** precisam fazer a mesma pergunta ("tem voz montada?") e
 * cada um deles escolhendo o campo por conta própria já deu errado uma vez: enquanto era
 * a chave da ElevenLabs, a pergunta era global e valia para as duas personas de uma vez.
 * Agora não vale mais — dá para ter o Jarvis com voz e o Ultron sem.
 */
export function vozDaPersona(settings: AppSettings): string {
  return settings.persona === 'ultron' ? settings.ttsVoiceUltron : settings.ttsVoiceJarvis
}

export const DEFAULT_SETTINGS: AppSettings = {
  anthropicApiKey: '',
  assistantName: 'Jarvis',
  persona: 'jarvis',
  ttsVoiceJarvis: '',
  ttsVoiceUltron: '',
  ollamaUrl: 'http://localhost:11434',
  ollamaModel: 'qwen2.5vl:3b',
  memoriaPath: '',
  braveApiKey: '',
  spotifyClientId: '',
  spotifyClientSecret: '',
  tuyaClientId: '',
  tuyaClientSecret: '',
  tuyaRegiao: 'us',
  webcamWidth: 0,
  webcamHeight: 0,
  webcamMirror: false,
  micDeviceName: '',
  logDetalhado: false,
}
