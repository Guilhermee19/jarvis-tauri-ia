'use client'

import { useEffect, useState } from 'react'

import { cn } from '@/lib/utils'
import { useCamerasStore } from '@/stores'
import type { Achado, Camera } from '@/types'

/**
 * Procurar câmeras na rede, para não ter que descobrir endereço nenhum.
 *
 * O app do fabricante esconde o IP — ele usa o P2P da nuvem e parte do princípio de que
 * ninguém precisa saber onde a câmera está. Aqui o endereço é a primeira coisa que falta,
 * e esta tela existe para que a resposta seja um clique em vez de uma caça ao tesouro no
 * roteador.
 *
 * O achado vira um cadastro **pré-preenchido**: endereço, tipo e URL do stream já vêm da
 * varredura, e o que sobra para digitar é só o nome — que é justamente a parte que só a
 * pessoa sabe, porque é como ela vai chamar a câmera em voz alta.
 */
export function VarreduraDeRede({
  onAdicionar,
  onFechar,
}: {
  onAdicionar: (camera: Camera) => void
  onFechar: () => void
}) {
  const prefixos = useCamerasStore((state) => state.prefixos)
  const achados = useCamerasStore((state) => state.achados)
  const varrendo = useCamerasStore((state) => state.varrendo)
  const carregarPrefixos = useCamerasStore((state) => state.carregarPrefixos)
  const varrer = useCamerasStore((state) => state.varrer)

  const [prefixo, setPrefixo] = useState('')

  useEffect(() => {
    void carregarPrefixos()
  }, [carregarPrefixos])

  // A primeira sugestão vira o valor inicial assim que ela chega — mas só enquanto o
  // campo estiver intocado, para não apagar o que a pessoa já digitou.
  const escolhido = prefixo || prefixos[0] || ''

  return (
    <div className="flex flex-col gap-3">
      <div>
        <p className="text-content text-[11px] font-medium">Procurar câmeras na rede</p>
        <p className="text-muted mt-0.5 text-[10px] leading-snug">
          Vou bater nas portas de vídeo de cada endereço da faixa e dizer o que responder.
          Leva alguns segundos.
        </p>
      </div>

      <div className="flex items-center gap-2">
        <label className="flex min-w-0 flex-1 items-center gap-1.5">
          <span className="text-muted shrink-0 text-[10px] tracking-[0.14em] uppercase">Faixa</span>
          <input
            value={escolhido}
            onChange={(evento) => setPrefixo(evento.target.value)}
            placeholder="192.168.18"
            className="border-border-soft bg-base text-content min-w-0 flex-1 rounded-md border px-2 py-1 text-[11px]"
          />
          <span className="text-muted shrink-0 text-[11px]">.1–254</span>
        </label>

        <button
          type="button"
          onClick={() => void varrer(escolhido)}
          disabled={varrendo || !escolhido.trim()}
          className="border-border-soft text-content shrink-0 rounded-md border px-2.5 py-1 text-[11px] disabled:opacity-60"
        >
          {varrendo ? 'Procurando…' : 'Procurar'}
        </button>
      </div>

      {/* As outras faixas como atalho. Aparecem só quando há mais de uma: numa casa com
          roteador em cascata, a do PC não é a das câmeras, e essa é a dica que evita uma
          varredura que não acharia nada. */}
      {prefixos.length > 1 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-muted text-[10px]">Também posso olhar:</span>
          {prefixos
            .filter((candidato) => candidato !== escolhido)
            .map((candidato) => (
              <button
                key={candidato}
                type="button"
                onClick={() => setPrefixo(candidato)}
                className="border-border-soft text-muted hover:text-content rounded-md border px-1.5 py-0.5 text-[10px]"
              >
                {candidato}
              </button>
            ))}
        </div>
      )}

      {varrendo && (
        <p className="text-muted text-[11px] leading-snug">
          Testando 254 endereços… a maioria não responde, e é a espera deles que leva o
          tempo.
        </p>
      )}

      {!varrendo && achados !== null && <Resultado achados={achados} onAdicionar={onAdicionar} />}

      <button
        type="button"
        onClick={onFechar}
        className="text-muted hover:text-content self-start text-[11px]"
      >
        ← voltar
      </button>
    </div>
  )
}

function Resultado({
  achados,
  onAdicionar,
}: {
  achados: Achado[]
  onAdicionar: (camera: Camera) => void
}) {
  if (achados.length === 0) {
    return (
      <p className="text-muted text-[11px] leading-snug">
        Não achei câmera nenhuma nessa faixa. Se o seu computador estiver numa rede
        diferente da das câmeras, tente a outra faixa — ou confira o endereço na lista de
        aparelhos do roteador.
      </p>
    )
  }

  return (
    <div className="flex flex-col gap-1.5">
      {achados.map((achado) => (
        <div
          key={achado.host}
          className="border-border-soft flex items-center gap-2 rounded-md border px-2.5 py-2"
        >
          <div className="min-w-0 flex-1">
            <p className="text-content truncate text-[11px]">
              <span className="tabular-nums">{achado.host}</span>
              {achado.jaCadastrada && <span className="text-muted"> · já cadastrada</span>}
            </p>
            <p className="text-muted truncate text-[10px]">{achado.descricao}</p>
            {achado.precisaSenha && (
              <p className="text-muted text-[10px]">Vai pedir usuário e senha.</p>
            )}
          </div>

          <button
            type="button"
            disabled={achado.jaCadastrada}
            onClick={() => onAdicionar(paraCadastro(achado))}
            className={cn(
              'border-border-soft text-content shrink-0 rounded-md border px-2 py-1 text-[11px]',
              'disabled:opacity-50',
            )}
          >
            {achado.jaCadastrada ? 'no catálogo' : 'Adicionar'}
          </button>
        </div>
      ))}
    </div>
  )
}

/**
 * O achado virando um cadastro em branco só onde a varredura não tinha como saber.
 *
 * `nome` fica vazio de propósito: é o que a pessoa vai FALAR ("mostra a garagem"), e
 * inventar um por ela — "IPCAM", "camera-179" — daria um nome que ninguém diz em voz alta
 * e que teria de ser corrigido logo depois.
 */
function paraCadastro(achado: Achado): Camera {
  return {
    id: '',
    nome: '',
    host: achado.host,
    tipo: achado.tipo,
    canal: 1,
    // `admin` é o usuário de fábrica dessas duas famílias, e é o palpite que acerta quase
    // sempre. Numa câmera que não pede senha ele é ignorado.
    usuario: achado.precisaSenha ? 'admin' : '',
    senha: '',
    rtspUrl: achado.rtspUrl,
    oculto: false,
    vigiar: false,
  }
}
