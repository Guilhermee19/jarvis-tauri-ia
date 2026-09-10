'use client'

import { useChatStore, useSensorStore, useSettingsStore } from '@/stores'

/** De quem é a vez enquanto a escuta está ligada. */
export type EscutaStatus = 'ouvindo' | 'pensando' | 'falando'

/**
 * A escuta contínua vista de fora — o que o HUD e o botão do microfone desenham.
 *
 * Não existe mais botão de "falar" nem de "conversar" no painel de chat: falar com ele
 * é ligar o microfone na barra e dizer o nome dele. O que sobrou aqui, então, é
 * LEITURA — ligar e desligar é do `sensorStore`, e o laço inteiro mora lá para
 * sobreviver a fechar a janelinha de conversa.
 *
 * O estado todo vem da store porque o dono do gravador precisa ser um só — o
 * microfone da bancada de diagnóstico e a escuta disputariam o dispositivo.
 */
export function useVoiceInput() {
  const isListening = useSensorStore((state) => state.isListening)
  const toggleListening = useSensorStore((state) => state.toggleListening)
  // O pico do microfone, para o HUD poder PROVAR que está ouvindo. Sem isso, mic
  // mudo no painel do Windows e mic funcionando são a mesma tela — e a diferença só
  // aparecia segundos depois, como "não ouvi nada".
  const level = useSensorStore((state) => state.micLevel)
  const ttsLevel = useSensorStore((state) => state.ttsLevel)
  const error = useSensorStore((state) => state.dictationError)
  const clearError = useSensorStore((state) => state.clearDictationError)

  // De quem é a vez. Derivado do chat, e não guardado no `sensorStore`, porque
  // pensar e falar são estados da RESPOSTA — copiá-los para cá criaria duas
  // versões da mesma verdade, e uma delas ficaria para trás.
  const isThinking = useChatStore((state) => state.isTyping)
  const isSpeaking = useChatStore((state) => state.isSpeaking)
  // O motor decide a calibração do medidor logo abaixo — os dois normalizam diferente.
  const motorDeVoz = useSettingsStore((state) => state.settings.ttsEngine)
  const status: EscutaStatus = isSpeaking ? 'falando' : isThinking ? 'pensando' : 'ouvindo'

  /**
   * O nível do áudio que importa AGORA, seja ele de entrada ou de saída.
   *
   * Uma grandeza só, porque quem desenha quer uma: o núcleo do HUD pulsa com o Jarvis
   * ouvindo e com ele falando, e as duas coisas nunca acontecem ao mesmo tempo — o
   * microfone fecha antes da fala começar, justamente para ele não ouvir a si mesmo.
   *
   * Continua na escala linear: a CURVA é de quem desenha, e os medidores deste projeto
   * aplicam `Math.sqrt` para tirar a fala do fundo da escala. Aplicá-la aqui esconderia
   * essa decisão de quem lê o componente.
   *
   * O que é feito aqui é outra coisa — **igualar as duas fontes**. Uma voz perto do
   * microfone encosta em 1; a fala sintetizada não chega lá. Sem compensar, o núcleo pulsa
   * visivelmente menos quando o Jarvis fala do que quando ele ouve, e essa diferença não
   * diz nada sobre o áudio.
   *
   * **Os dois números saíram de medir**, e são bem diferentes:
   *
   * - **Piper: 1,0.** Ele normaliza a saída, e a frase de teste bateu pico exatamente 1,000.
   * - **Chatterbox: 0,28.** Uma frase de 4,4 s gerada em WAV bateu 0,264 — e ainda depende
   *   do clipe, porque o modelo clona o VOLUME junto com a voz.
   *
   * Por isso a constante segue o motor, e não é um número só. Usar o 0,28 com o Piper faria
   * o núcleo saturar em toda sílaba; usar 1,0 com o Chatterbox o deixaria quase parado. É a
   * mesma armadilha que o 0,6 da ElevenLabs já causou uma vez, e o que a evita é ela ser
   * derivada de `ttsEngine` em vez de escrita à mão.
   *
   * Se trocar o clipe do Chatterbox e o núcleo pulsar de menos, o número novo sai de
   * `cargo test --lib -- --ignored --nocapture fala_de_verdade`, que imprime o pico.
   */
  const PICO_TIPICO_DA_FALA = motorDeVoz === 'piper' ? 1 : 0.28
  const nivelDeAudio = isSpeaking ? Math.min(1, ttsLevel / PICO_TIPICO_DA_FALA) : level

  return {
    isListening,
    toggleListening,
    nivelDeAudio,
    status,
    level,
    error,
    clearError,
  }
}
