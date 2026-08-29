import type { Recording, Voice } from '@/types'
import { call, isTauriRuntime } from './client'

/** Wrappers de `src-tauri/src/commands/voice.rs`. */

/**
 * Enquanto grava, o backend emite `JarvisEvent.MicLevel` ~20×/s com o pico do
 * intervalo — é o que alimenta o medidor de volume.
 */
export function startRecording(): Promise<void> {
  return call<void>('start_recording')
}

export function listInputDevices(): Promise<string[]> {
  return call<string[]>('list_input_devices')
}

/** Fecha o microfone e devolve o WAV gravado em disco. */
export function stopRecording(): Promise<Recording> {
  return call<Recording>('stop_recording')
}

export function isRecording(): Promise<boolean> {
  return call<boolean>('is_recording')
}

/**
 * Transcreve a última gravação com o Whisper local. Separado do `stopRecording` de
 * propósito: a bancada testa o microfone sem esperar o Whisper, e quem fala com o
 * chat encadeia os dois.
 *
 * A PRIMEIRA chamada sobe o `whisper-server` e carrega o modelo — conte alguns
 * segundos a mais nela do que nas seguintes.
 */
export function transcribe(): Promise<string> {
  return call<string>('transcribe')
}

/**
 * Abre o seletor de arquivos nativo e devolve o caminho escolhido, ou `null` se
 * cancelaram.
 *
 * Mora aqui, com os outros wrappers, e não solto no componente: a regra da casa é que
 * TODO acesso ao backend passa por `lib/tauri`, e o diálogo é backend — quem o desenha é
 * o sistema, não o React.
 */
export async function escolherClipeDeVoz(): Promise<string | null> {
  if (!isTauriRuntime()) return null

  const { open } = await import('@tauri-apps/plugin-dialog')
  const escolhido = await open({
    multiple: false,
    directory: false,
    filters: [{ name: 'Áudio', extensions: ['wav', 'mp3'] }],
  })

  return typeof escolhido === 'string' ? escolhido : null
}

/**
 * Os clipes de voz cadastrados no servidor local.
 *
 * Como o `transcribe`, a PRIMEIRA chamada pode subir o servidor e carregar o modelo —
 * conte bem mais que alguns segundos nela.
 */
export function listVoices(): Promise<Voice[]> {
  return call<Voice[]>('list_voices')
}

/**
 * Manda um arquivo de áudio para o servidor virar voz clonável, e devolve o nome com que
 * ele ficou guardado lá.
 *
 * O nome de volta é o que importa: o servidor higieniza o nome do arquivo, então gravar o
 * que foi escolhido no disco em vez do que ele respondeu daria uma voz que não existe.
 */
export function uploadVoiceReference(caminho: string): Promise<string> {
  return call<string>('upload_voice_reference', { caminho })
}

/**
 * `voiceId` é opcional de propósito: sem ele o backend usa o clipe da persona ativa.
 * É essa assinatura que o resto do app usa — `speakText(resposta)` e pronto, sem precisar
 * saber onde os clipes moram.
 */
export function speakText(text: string, voiceId?: string): Promise<void> {
  return call<void>('speak_text', { text, voiceId: voiceId ?? null })
}

/**
 * Cala a fala em andamento. Vale também durante a SÍNTESE — desligar o modo conversa
 * enquanto o modelo ainda está gerando não pode deixar a frase chegar e tocar depois.
 */
export function stopSpeaking(): Promise<void> {
  return call<void>('stop_speaking')
}
