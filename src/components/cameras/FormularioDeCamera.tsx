'use client'

import { useState } from 'react'

import { cn } from '@/lib/utils'
import { useCamerasStore } from '@/stores'
import type { Camera, TipoDeCamera } from '@/types'

/** Um cadastro em branco. `dvr` como padrão porque é o caso que precisa de mais campos. */
function emBranco(): Camera {
  return {
    id: '',
    nome: '',
    host: '',
    tipo: 'dvr',
    canal: 1,
    usuario: 'admin',
    senha: '',
    rtspUrl: '',
    oculto: false,
    vigiar: false,
  }
}

/**
 * O id que vira o `src` do go2rtc, derivado do nome.
 *
 * Derivado e não digitado: ninguém quer inventar um identificador, e um id com espaço ou
 * acento quebraria a query string e o YAML. Digitar "Portão dos Fundos" e receber
 * `portao-dos-fundos` é o comportamento esperado.
 */
function idDoNome(nome: string): string {
  return nome
    .normalize('NFD')
    // Os acentos, escritos como escape em vez de literais: uma faixa de combining marks
    // colada crua no arquivo é invisível no editor e some num salvamento em outra
    // codificação.
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
}

/**
 * Cadastrar uma câmera.
 *
 * Dois caminhos, porque as duas famílias de câmera se cadastram de formas diferentes: a
 * ONVIF **se apresenta** (o botão "Perguntar à câmera" traz modelo e URL do stream), e o
 * DVR não fala ONVIF — nele o que identifica cada câmera é o número do canal, e as
 * credenciais são obrigatórias.
 */
export function FormularioDeCamera({
  inicial,
  edicao = false,
  onPronto,
}: {
  /** Um cadastro pré-preenchido — da varredura da rede, ou o que já existe no catálogo. */
  inicial?: Camera
  /**
   * `true` só quando se está mexendo numa câmera que JÁ existe.
   *
   * Separado de `inicial` porque as duas coisas deixaram de andar juntas quando a
   * varredura passou a entregar um cadastro pronto: ele vem preenchido e mesmo assim é
   * novo. Derivar `edicao` de `inicial` faria a câmera achada na rede reusar um id vazio
   * em vez de ganhar o seu.
   */
  edicao?: boolean
  onPronto: () => void
}) {
  const [camera, setCamera] = useState<Camera>(inicial ?? emBranco())
  const [sondando, setSondando] = useState(false)
  const [aviso, setAviso] = useState<string | null>(null)
  const [salvando, setSalvando] = useState(false)

  const salvar = useCamerasStore((state) => state.salvar)
  const sondar = useCamerasStore((state) => state.sondar)

  const editando = edicao
  const campo = <K extends keyof Camera>(chave: K, valor: Camera[K]) =>
    setCamera((atual) => ({ ...atual, [chave]: valor }))

  async function perguntarACamera() {
    if (!camera.host.trim()) {
      setAviso('Preencha o endereço primeiro.')
      return
    }

    setSondando(true)
    setAviso(null)
    try {
      const achado = await sondar(camera.host.trim())
      setCamera((atual) => ({
        ...atual,
        tipo: 'onvif',
        // A URL que a câmera disse ganha do palpite — é o ponto inteiro de perguntar.
        rtspUrl: achado.rtspUrl,
      }))
      setAviso(`Achei: ${achado.descricao}`)
    } catch {
      // O motivo é descartado de propósito: o DVR não fala ONVIF, e cadastrá-lo à mão é
      // o caminho normal dele. Mostrar "erro de rede" aqui daria a impressão de que o
      // cadastro falhou, quando o que houve foi uma pergunta que não se aplicava.
      setAviso(
        'Essa não respondeu por ONVIF. Se for um DVR (XMEye), preencha canal, usuário e senha à mão.',
      )
    } finally {
      setSondando(false)
    }
  }

  async function enviar(evento: React.FormEvent) {
    evento.preventDefault()

    const nome = camera.nome.trim()
    if (!nome || !camera.host.trim()) {
      setAviso('Nome e endereço são obrigatórios.')
      return
    }

    setSalvando(true)
    try {
      // O id só é derivado na criação: mudá-lo numa edição criaria uma câmera nova e
      // deixaria a antiga órfã no arquivo.
      await salvar({ ...camera, nome, id: editando ? camera.id : idDoNome(nome) })
      onPronto()
    } catch (erro) {
      setAviso(erro instanceof Error ? erro.message : String(erro))
    } finally {
      setSalvando(false)
    }
  }

  return (
    <form onSubmit={(evento) => void enviar(evento)} className="flex flex-col gap-2.5">
      <div className="grid grid-cols-2 gap-2">
        <Campo
          rotulo="Nome"
          dica="É por ele que você chama: “mostra a garagem”."
          valor={camera.nome}
          onChange={(valor) => campo('nome', valor)}
          placeholder="garagem"
        />
        <Campo
          rotulo="Endereço na rede"
          valor={camera.host}
          onChange={(valor) => campo('host', valor)}
          placeholder="192.168.18.249"
        />
      </div>

      <div className="flex items-center gap-2">
        <label className="text-muted flex items-center gap-1.5 text-[11px]">
          Tipo
          <select
            value={camera.tipo}
            onChange={(evento) => campo('tipo', evento.target.value as TipoDeCamera)}
            className="border-border-soft bg-base text-content rounded-md border px-2 py-1 text-[11px]"
          >
            <option value="dvr">DVR (XMEye)</option>
            <option value="onvif">Câmera ONVIF (V380)</option>
          </select>
        </label>

        <button
          type="button"
          onClick={() => void perguntarACamera()}
          disabled={sondando}
          className={cn(
            'border-border-soft text-muted hover:text-content rounded-md border px-2 py-1 text-[11px]',
            'disabled:opacity-60',
          )}
        >
          {sondando ? 'Perguntando…' : 'Perguntar à câmera'}
        </button>
      </div>

      {/* O canal só existe no DVR: um endereço com várias câmeras dentro. Mostrá-lo na
          ONVIF sugeriria que ela tem canais, e ela não tem. */}
      {camera.tipo === 'dvr' && (
        <Campo
          rotulo="Canal"
          dica="Qual câmera dentro do DVR. Começa em 1."
          valor={String(camera.canal)}
          onChange={(valor) => campo('canal', Number(valor) || 1)}
          tipo="number"
        />
      )}

      <div className="grid grid-cols-2 gap-2">
        <Campo
          rotulo="Usuário"
          valor={camera.usuario}
          onChange={(valor) => campo('usuario', valor)}
          placeholder="admin"
        />
        <Campo
          rotulo="Senha"
          valor={camera.senha}
          onChange={(valor) => campo('senha', valor)}
          tipo="password"
        />
      </div>

      <Campo
        rotulo="URL RTSP"
        dica="Vazio = eu monto sozinho. Preenchido, ganha do meu palpite."
        valor={camera.rtspUrl}
        onChange={(valor) => campo('rtspUrl', valor)}
        placeholder="(automático)"
      />

      <label className="text-muted flex items-center gap-2 text-[11px]">
        <input
          type="checkbox"
          checked={camera.vigiar}
          onChange={(evento) => campo('vigiar', evento.target.checked)}
        />
        Avisar quando algo se mexer nesta câmera
      </label>

      {aviso && <p className="text-muted text-[11px] leading-snug">{aviso}</p>}

      <div className="flex items-center gap-2">
        <button
          type="submit"
          disabled={salvando}
          className="border-border-soft text-content rounded-md border px-3 py-1 text-[11px] disabled:opacity-60"
        >
          {salvando ? 'Salvando…' : editando ? 'Salvar' : 'Adicionar'}
        </button>
        <button
          type="button"
          onClick={onPronto}
          className="text-muted hover:text-content px-2 py-1 text-[11px]"
        >
          Cancelar
        </button>
      </div>
    </form>
  )
}

function Campo({
  rotulo,
  dica,
  valor,
  onChange,
  placeholder,
  tipo = 'text',
}: {
  rotulo: string
  dica?: string
  valor: string
  onChange: (valor: string) => void
  placeholder?: string
  tipo?: string
}) {
  return (
    <label className="flex min-w-0 flex-col gap-1">
      <span className="text-muted text-[10px] tracking-[0.14em] uppercase">{rotulo}</span>
      <input
        type={tipo}
        value={valor}
        placeholder={placeholder}
        onChange={(evento) => onChange(evento.target.value)}
        className="border-border-soft bg-base text-content rounded-md border px-2 py-1 text-[11px]"
      />
      {dica && <span className="text-muted text-[10px] leading-snug">{dica}</span>}
    </label>
  )
}
