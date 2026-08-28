import type {
  AjusteLuz,
  Aparelho,
  Controle,
  DetalheAparelho,
  EstadoAparelho,
  Importado,
  Tecla,
  Varredura,
} from '@/types'
import { call } from './client'

/** Wrappers de `src-tauri/src/commands/casa.rs`. */

/**
 * Escuta a rede à procura de aparelhos Tuya (Positivo, EKAZA e companhia).
 *
 * **Demora alguns segundos e isso não é lentidão.** Os aparelhos se anunciam sozinhos
 * de tempos em tempos, e não há como pedir que falem antes da hora — a chamada é uma
 * janela de escuta, não uma consulta. Quem chama precisa mostrar que está esperando.
 */
export function discoverDevices(): Promise<Varredura> {
  return call<Varredura>('discover_devices')
}

/**
 * O que já se conhece, sem encostar na rede — responde na hora.
 *
 * É o que o painel mostra no instante em que abre, em vez de dez segundos de tela vazia
 * até a primeira varredura terminar.
 */
export function knownDevices(): Promise<Aparelho[]> {
  return call<Aparelho[]>('known_devices')
}

/**
 * Busca nome e chave de controle na nuvem da Tuya e guarda em disco. Devolve quantos.
 *
 * `semente` é o id de um aparelho que a varredura já viu: a Tuya lista os aparelhos de um
 * USUÁRIO, e o usuário se descobre perguntando por um aparelho conhecido. É o que evita
 * pedir para alguém digitar um id de 22 caracteres.
 */
export function importTuyaDevices(semente: string): Promise<Importado[]> {
  return call<Importado[]>('import_tuya_devices', { semente })
}

/**
 * Liga ou desliga um aparelho, direto na rede local — sem nuvem e sem internet.
 *
 * `ip` e `versao` vão daqui porque a varredura mais recente vive na tela: o backend não
 * guarda o retrato da rede, e um IP guardado envelhece calado quando o roteador
 * redistribui os endereços. A chave é a única coisa que ele guarda.
 */
export function setDevicePower(
  id: string,
  ip: string,
  versao: string,
  ligado: boolean,
): Promise<EstadoAparelho> {
  return call<EstadoAparelho>('set_device_power', { id, ip, versao, ligado })
}

/**
 * Tudo o que o aparelho sabe dizer sobre si: estado, o que ele aceita de ajuste, e os
 * data points crus.
 *
 * Custa uma conexão TCP e um aperto de mão, então só é chamado quando alguém abre os
 * detalhes — não a cada varredura.
 */
export function deviceState(id: string, ip: string, versao: string): Promise<DetalheAparelho> {
  return call<DetalheAparelho>('device_state', { id, ip, versao })
}

/** Muda cor, brilho ou temperatura de uma lâmpada, e devolve como ela ficou. */
export function setLight(
  id: string,
  ip: string,
  versao: string,
  ajuste: AjusteLuz,
): Promise<DetalheAparelho> {
  return call<DetalheAparelho>('set_light', { id, ip, versao, ajuste })
}

/** Tira um aparelho da lista principal, ou devolve para ela. Só a tela muda. */
export function setDeviceHidden(id: string, oculto: boolean): Promise<void> {
  return call<void>('set_device_hidden', { id, oculto })
}

/**
 * As teclas de um controle de infravermelho — as da TV, as do ar-condicionado.
 *
 * Vem da nuvem porque é lá que elas moram: o emissor guarda zero códigos, ele só emite o
 * que mandarem. É a mesma razão de a TV não aparecer na varredura da rede.
 */
export function irKeys(emissor: string, remoto: string): Promise<Controle> {
  return call<Controle>('ir_keys', { emissor, remoto })
}

/**
 * Aperta uma tecla do controle.
 *
 * **É o único comando do app que precisa de internet.** O código infravermelho de "ligar
 * a TV" mora na biblioteca da Tuya, não no emissor — ele não tem o que mandar até alguém
 * contar qual é o código.
 */
export function sendIrKey(
  emissor: string,
  remoto: string,
  categoria: number,
  tecla: Tecla,
): Promise<void> {
  return call<void>('send_ir_key', { emissor, remoto, categoria, tecla })
}
