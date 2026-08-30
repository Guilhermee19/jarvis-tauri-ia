'use client'

import { useEffect, useState } from 'react'

import { FormularioDeCamera } from './FormularioDeCamera'
import { StreamDeCamera } from './StreamDeCamera'
import { VarreduraDeRede } from './VarreduraDeRede'
import { cn } from '@/lib/utils'
import { useCamerasStore } from '@/stores'
import type { Camera, Direcao } from '@/types'

/**
 * As câmeras de segurança da casa.
 *
 * Dois estados de tela, e a diferença é o que se quer fazer: a **grade** é para bater o
 * olho em tudo, e o **foco** é para olhar uma. Clicar num cartão troca entre os dois. O
 * PTZ só aparece no foco — botões de virar em seis miniaturas seriam ruído, e virar a
 * câmera errada é o erro que eles causariam.
 */
export function CamerasPanel() {
  const cameras = useCamerasStore((state) => state.cameras)
  const ligando = useCamerasStore((state) => state.ligando)
  const erro = useCamerasStore((state) => state.erro)
  const emFoco = useCamerasStore((state) => state.emFoco)
  const carregar = useCamerasStore((state) => state.carregar)
  const ligar = useCamerasStore((state) => state.ligar)
  const focar = useCamerasStore((state) => state.focar)

  /**
   * O que a tela está fazendo além de mostrar imagem.
   *
   * Um estado só, e não três booleanos: as três telas são mutuamente exclusivas, e com
   * booleanos separados existiria um estado "varrendo e editando ao mesmo tempo" que não
   * quer dizer nada e que alguém acabaria criando sem querer.
   */
  const [tarefa, setTarefa] = useState<
    | { modo: 'nenhuma' }
    | { modo: 'varrendo' }
    | { modo: 'cadastro'; inicial?: Camera }
    | { modo: 'edicao'; camera: Camera }
  >({ modo: 'nenhuma' })

  const fecharTarefa = () => setTarefa({ modo: 'nenhuma' })

  // Duas chamadas com propósitos diferentes: `carregar` responde na hora e enche a tela;
  // `ligar` sobe um processo e leva segundos. Fazer as duas juntas é o que evita a janela
  // ficar vazia enquanto o go2rtc nasce.
  useEffect(() => {
    void carregar()
    void ligar()
  }, [carregar, ligar])

  const visiveis = cameras.filter((camera) => !camera.oculto)
  const focada = emFoco ? cameras.find((camera) => camera.id === emFoco) : null

  if (tarefa.modo !== 'nenhuma') {
    return (
      <div className="scroll-thin flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-3 py-3">
        {tarefa.modo === 'varrendo' ? (
          <VarreduraDeRede
            // O achado entra no formulário já preenchido: sobra só o nome, que é a parte
            // que a varredura não tem como saber.
            onAdicionar={(inicial) => setTarefa({ modo: 'cadastro', inicial })}
            onFechar={fecharTarefa}
          />
        ) : (
          <FormularioDeCamera
            inicial={tarefa.modo === 'edicao' ? tarefa.camera : tarefa.inicial}
            edicao={tarefa.modo === 'edicao'}
            onPronto={fecharTarefa}
          />
        )}
      </div>
    )
  }

  return (
    <div className="scroll-thin flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-3 py-3">
      {erro && (
        <p className="border-border-soft text-muted rounded-md border px-2.5 py-2 text-[11px] leading-snug">
          {erro}
        </p>
      )}

      <Alertas onFocar={focar} />

      {cameras.length === 0 ? (
        <Vazio
          onProcurar={() => setTarefa({ modo: 'varrendo' })}
          onAdicionar={() => setTarefa({ modo: 'cadastro' })}
          ligando={ligando}
        />
      ) : focada ? (
        <Foco
          camera={focada}
          onVoltar={() => focar(null)}
          onEditar={() => setTarefa({ modo: 'edicao', camera: focada })}
        />
      ) : (
        <Grade cameras={visiveis} onFocar={focar} />
      )}

      {cameras.length > 0 && !focada && (
        <div className="flex items-center gap-2">
          {/* Procurar vem primeiro: com a rede varrida, o cadastro sai quase pronto, e
              digitar endereço à mão passa a ser a exceção. */}
          <button
            type="button"
            onClick={() => setTarefa({ modo: 'varrendo' })}
            className="border-border-soft text-content rounded-md border px-2.5 py-1 text-[11px]"
          >
            Procurar na rede
          </button>
          <button
            type="button"
            onClick={() => setTarefa({ modo: 'cadastro' })}
            className="text-muted hover:text-content text-[11px]"
          >
            adicionar à mão
          </button>
        </div>
      )}
    </div>
  )
}

/**
 * O que se mexeu recentemente.
 *
 * Fica no topo e some sozinho quando limpo — é "o que acabou de acontecer", não um
 * histórico. O histórico de verdade está na memória do Jarvis, onde dá para perguntar
 * depois. Clicar num alerta leva à câmera que o gerou, que é a única coisa que se quer
 * fazer ao ler um.
 */
function Alertas({ onFocar }: { onFocar: (id: string) => void }) {
  const alertas = useCamerasStore((state) => state.alertas)
  const limpar = useCamerasStore((state) => state.limparAlertas)

  if (alertas.length === 0) return null

  return (
    <div className="border-border-soft flex flex-col gap-1.5 rounded-md border px-2.5 py-2">
      <div className="flex items-center justify-between">
        <span className="text-muted text-[10px] tracking-[0.14em] uppercase">Movimento</span>
        <button type="button" onClick={limpar} className="text-muted hover:text-content text-[10px]">
          limpar
        </button>
      </div>
      {alertas.map((alerta) => (
        <button
          key={`${alerta.camera}-${alerta.quando}`}
          type="button"
          onClick={() => onFocar(alerta.camera)}
          className="text-content text-left text-[11px] leading-snug"
        >
          <span className="text-muted tabular-nums">{hora(alerta.quando)}</span>{' '}
          <span className="font-medium">{alerta.nome}</span> — {alerta.resposta}
        </button>
      ))}
    </div>
  )
}

function hora(quando: number): string {
  return new Date(quando).toLocaleTimeString('pt-BR', {
    hour: '2-digit',
    minute: '2-digit',
  })
}

function Grade({
  cameras,
  onFocar,
}: {
  cameras: Camera[]
  onFocar: (id: string) => void
}) {
  return (
    <div className="grid grid-cols-2 gap-2">
      {cameras.map((camera) => (
        <button
          key={camera.id}
          type="button"
          onClick={() => onFocar(camera.id)}
          className="border-border-soft group relative overflow-hidden rounded-md border text-left"
        >
          <StreamDeCamera camera={camera} className="aspect-video h-full w-full" />
          {/* O nome sobre a imagem, e não abaixo: numa grade de quatro, alinhar as
              imagens importa mais que alinhar os rótulos. */}
          <span className="from-base/90 absolute inset-x-0 bottom-0 bg-linear-to-t to-transparent px-2 py-1.5 text-[11px]">
            {camera.nome}
          </span>
        </button>
      ))}
    </div>
  )
}

function Foco({
  camera,
  onVoltar,
  onEditar,
}: {
  camera: Camera
  onVoltar: () => void
  onEditar: () => void
}) {
  const mover = useCamerasStore((state) => state.mover)
  const remover = useCamerasStore((state) => state.remover)

  return (
    <div className="flex flex-col gap-2">
      <StreamDeCamera
        camera={camera}
        className="border-border-soft aspect-video w-full rounded-md border"
      />

      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={onVoltar}
          className="text-muted hover:text-content text-[11px]"
        >
          ← todas
        </button>
        <span className="text-content flex-1 truncate text-[11px]">{camera.nome}</span>
        <button type="button" onClick={onEditar} className="text-muted hover:text-content text-[11px]">
          editar
        </button>
        <button
          type="button"
          onClick={() => void remover(camera.id)}
          className="text-muted hover:text-content text-[11px]"
        >
          remover
        </button>
      </div>

      {/* O PTZ só existe na ONVIF. Mostrar as setas apagadas num canal de DVR sugeriria
          que ele se mexe e está com defeito — o certo é dizer que ele não tem isso. */}
      {camera.tipo === 'onvif' ? (
        <ControlePtz onMover={(direcao) => void mover(camera.id, direcao)} />
      ) : (
        <p className="text-muted text-[10px]">Câmera de DVR: não tem controle de movimento.</p>
      )}
    </div>
  )
}

const SETAS: { direcao: Direcao; rotulo: string; seta: string }[] = [
  { direcao: 'cima', rotulo: 'Virar para cima', seta: '↑' },
  { direcao: 'esquerda', rotulo: 'Virar para a esquerda', seta: '←' },
  { direcao: 'direita', rotulo: 'Virar para a direita', seta: '→' },
  { direcao: 'baixo', rotulo: 'Virar para baixo', seta: '↓' },
]

function ControlePtz({ onMover }: { onMover: (direcao: Direcao) => void }) {
  return (
    <div className="flex items-center justify-center gap-1.5">
      {SETAS.map(({ direcao, rotulo, seta }) => (
        <button
          key={direcao}
          type="button"
          onClick={() => onMover(direcao)}
          aria-label={rotulo}
          title={rotulo}
          className={cn(
            'border-border-soft text-muted hover:text-content flex h-7 w-7 items-center justify-center rounded-md border',
            'active:scale-[0.94] motion-safe:transition-transform',
          )}
        >
          {seta}
        </button>
      ))}
    </div>
  )
}

function Vazio({
  onProcurar,
  onAdicionar,
  ligando,
}: {
  onProcurar: () => void
  onAdicionar: () => void
  ligando: boolean
}) {
  return (
    <div className="flex flex-col items-start gap-2 py-4">
      <p className="text-muted text-[11px] leading-snug">
        Nenhuma câmera cadastrada ainda. Posso procurar sozinho na sua rede — acho DVR do
        XMEye e câmeras ONVIF (as do V380) e já trago o endereço preenchido.
      </p>
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={onProcurar}
          disabled={ligando}
          className="border-border-soft text-content rounded-md border px-2.5 py-1 text-[11px] disabled:opacity-60"
        >
          {ligando ? 'Preparando…' : 'Procurar na rede'}
        </button>
        <button
          type="button"
          onClick={onAdicionar}
          className="text-muted hover:text-content text-[11px]"
        >
          adicionar à mão
        </button>
      </div>
    </div>
  )
}
