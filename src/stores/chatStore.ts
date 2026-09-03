import { create } from 'zustand'
import {
  announce,
  clearHistory,
  getHistory,
  sendMessage,
  speakText,
  stopSpeaking,
} from '@/lib/tauri'
import { useSettingsStore } from './settingsStore'
import { vozDaPersona } from '@/types'
import type { ChatMessage } from '@/types'

/**
 * O histórico canônico é do backend (`AppState` no Rust). Esta store é um espelho
 * para a UI — por isso `loadHistory` sobrescreve tudo em vez de fazer merge.
 */
interface ChatState {
  messages: ChatMessage[]
  isTyping: boolean
  /**
   * Ele está falando a resposta agora.
   *
   * Mora aqui, e não no `sensorStore` junto do microfone, porque a fala acompanha a
   * RESPOSTA — vale para o que foi digitado igual ao que foi falado. O modo conversa
   * lê esta flag em vez de manter uma cópia própria.
   */
  isSpeaking: boolean
  /**
   * O id da bolha que está sendo escrita AGORA, frase a frase, ou `null` fora de um
   * turno.
   *
   * É uma bolha otimista como a do usuário: o backend manda cada frase por evento
   * enquanto o modelo ainda escreve, e o `loadHistory` do fim troca tudo pela versão com
   * o id de verdade. Guardar o id, e não o texto, é o que deixa a frase seguinte achar a
   * bolha certa sem depender de ela ser a última da lista.
   */
  respostaEmCurso: string | null
  /**
   * O crachá do turno que está na tela, ou `null` fora de um.
   *
   * Só as frases carimbadas com ele são desenhadas. Existe por causa da interrupção:
   * mandar uma pergunta nova enquanto ele responde a anterior corta a FALA, mas o texto
   * velho continua chegando do modelo por alguns segundos — e sem o crachá ele entraria
   * na bolha da pergunta nova.
   */
  turnoAtual: string | null
  error: string | null
  loadHistory: () => Promise<void>
  /**
   * Envia e espera — e o comando só volta quando ele calou.
   *
   * **Quem fala é o backend**, desde que a resposta passou a sair em fluxo: as frases
   * nascem lá, uma a uma, e mandá-las de volta para cá só para reenviá-las ao motor de
   * voz custaria uma ida e volta por frase. O que chega aqui são as mesmas frases como
   * TEXTO (veja {@link ChatState.receberFrase}), no mesmo passo da fala.
   *
   * O `await` até o fim continua valendo, e é ele que deixa o laço da conversa saber
   * quando pode reabrir o microfone — só que agora quem espera a última frase calar é o
   * `send_message` do Rust.
   *
   * Devolve o texto da resposta, ou string vazia se nada foi enviado ou algo falhou.
   */
  send: (content: string) => Promise<string>
  /**
   * Uma frase da resposta, recém-saída do modelo e a caminho da caixa de som.
   *
   * Chamado pelo `useSensorEvents` a cada `JarvisEvent.ReplyChunk`. A primeira frase
   * abre a bolha e desliga o "digitando"; as seguintes crescem a mesma bolha. Frase de
   * turno que não é o da tela é descartada — veja {@link ChatState.turnoAtual}.
   */
  receberFrase: (turno: string, frase: string) => void
  /**
   * O Jarvis dizendo algo por iniciativa própria — hoje, a saudação de quando o app abre.
   *
   * Diferente do {@link ChatState.send} em duas coisas: não há mensagem do usuário antes,
   * e não passa pelo agente (não há o que interpretar numa frase que o app compôs). O que
   * ele compartilha é o que importa: a frase é gravada no backend e falada com a mesma
   * voz, pelo mesmo caminho — sem isso, a saudação soaria de outro jeito que o resto.
   */
  anunciar: (texto: string) => Promise<void>
  clear: () => Promise<void>
}

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

/**
 * Fala uma frase INTEIRA, e só volta quando ela termina.
 *
 * Sobrou para o {@link ChatState.anunciar}: a saudação é composta pelo próprio app, sai
 * pronta de uma vez e não passa pelo modelo, então não há frase a esperar. A resposta do
 * agente segue outro caminho — nasce em pedaços no Rust e é falada de lá, frase a frase.
 *
 * Sem clipe de voz cadastrado ele fica calado e SEM ERRO: voz é opcional, e um aviso
 * vermelho a cada mensagem digitada seria ruído por uma coisa que ninguém pediu. Quem
 * liga o modo conversa aí sim recebe a recusa na hora do clique, porque ali a voz é o
 * ponto.
 *
 * Esse silêncio importa mais do que importava: sem clipe, tentar falar subiria o servidor
 * de voz — segundos de modelo carregando — para no fim não ter voz nenhuma para clonar.
 */
async function falar(texto: string, set: (state: Partial<ChatState>) => void, falando: boolean) {
  if (!texto.trim()) return
  if (!temVoz()) return

  // Mandar uma mensagem nova enquanto ele fala a anterior CORTA a anterior. Duas
  // falas sobrepostas seriam ininteligíveis, e a resposta que interessa é a última.
  // ponytail: corta pela flag, então a anterior morre em até 100 ms — sobreposição
  // só apareceria se a síntese da nova voltasse mais rápido que isso, o que a rede
  // não permite.
  if (falando) await stopSpeaking().catch(() => undefined)

  set({ isSpeaking: true })
  try {
    await speakText(texto)
  } catch (error) {
    set({ error: describeError(error) })
  } finally {
    set({ isSpeaking: false })
  }
}

/** Se a persona ativa tem voz escolhida. Sem ela o backend não fala, e nem tenta. */
function temVoz(): boolean {
  return vozDaPersona(useSettingsStore.getState().settings).trim() !== ''
}

export const useChatStore = create<ChatState>((set, get) => ({
  messages: [],
  isTyping: false,
  isSpeaking: false,
  respostaEmCurso: null,
  turnoAtual: null,
  error: null,

  loadHistory: async () => {
    try {
      set({ messages: await getHistory(), error: null })
    } catch (error) {
      set({ error: describeError(error) })
    }
  },

  send: async (content: string) => {
    const trimmed = content.trim()
    if (!trimmed || get().isTyping) return ''

    // Mandar uma mensagem nova enquanto ele fala a anterior CORTA a anterior. Duas falas
    // sobrepostas seriam ininteligíveis, e a resposta que interessa é a última — e agora
    // isso vale para a fila inteira: o Rust larga as frases que ainda não tocaram.
    if (get().isSpeaking) await stopSpeaking().catch(() => undefined)

    // Bolha otimista: o backend gera o id definitivo, que chega no próximo `loadHistory`.
    const optimistic: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content: trimmed,
      timestamp: Date.now(),
    }
    // O crachá deste turno. As frases voltam carimbadas com ele, e é o que faz uma
    // resposta interrompida parar de escrever quando a próxima pergunta já está na tela —
    // cortar a fala não corta o texto, que continua chegando do modelo por uns segundos.
    const turno = crypto.randomUUID()

    set((state) => ({
      messages: [...state.messages, optimistic],
      isTyping: true,
      turnoAtual: turno,
      respostaEmCurso: null,
      error: null,
    }))

    try {
      // Só volta quando ele calou: as frases já foram aparecendo pelo
      // `receberFrase` e saindo pela caixa de som ao mesmo tempo.
      const { message } = await sendMessage(trimmed, turno)

      // O turno que foi interrompido chega aqui com a tela já pertencendo a outro. Ele
      // não apaga o "digitando" nem recarrega nada: quem manda é o turno vigente.
      if (get().turnoAtual !== turno) return message.content

      set({ isTyping: false, isSpeaking: false, respostaEmCurso: null })

      // Uma jogada do agente pode empurrar DUAS mensagens no histórico: o log do
      // gatilho e a resposta. Recarregar em vez de dar append mantém o espelho fiel
      // — e de quebra troca as duas bolhas otimistas pelas versões com o id do backend.
      await get().loadHistory()
      return message.content
    } catch (error) {
      if (get().turnoAtual !== turno) return ''

      set({
        isTyping: false,
        isSpeaking: false,
        respostaEmCurso: null,
        error: describeError(error),
      })
      return ''
    }
  },

  receberFrase: (turno: string, frase: string) => {
    const texto = frase.trim()
    if (!texto || turno !== get().turnoAtual) return

    const emCurso = get().respostaEmCurso
    if (emCurso) {
      set((state) => ({
        messages: state.messages.map((message) =>
          message.id === emCurso ? { ...message, content: `${message.content} ${texto}` } : message,
        ),
      }))
      return
    }

    const bolha: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'assistant',
      content: texto,
      timestamp: Date.now(),
    }

    // O `isSpeaking` acompanha a voz, então segue o MESMO portão do backend: sem clipe
    // escolhido ele responde calado, e acender "falando" no HUD seria mentira.
    set((state) => ({
      messages: [...state.messages, bolha],
      respostaEmCurso: bolha.id,
      isTyping: false,
      isSpeaking: temVoz(),
    }))
  },

  anunciar: async (texto: string) => {
    const frase = texto.trim()
    if (!frase) return

    try {
      await announce(frase)
      await get().loadHistory()
    } catch (error) {
      // A gravação falhou, mas a fala ainda vale: ouvir "bom dia, Guilherme" sem a linha
      // na conversa é melhor que um erro vermelho na abertura do app.
      set({ error: describeError(error) })
    }

    // Depois do `loadHistory`, pela mesma razão do `send`: a frase aparece escrita e
    // ENTÃO ele começa a falar.
    await falar(frase, set, get().isSpeaking)
  },

  clear: async () => {
    try {
      await clearHistory()
      set({ messages: [], respostaEmCurso: null, error: null })
    } catch (error) {
      set({ error: describeError(error) })
    }
  },
}))
