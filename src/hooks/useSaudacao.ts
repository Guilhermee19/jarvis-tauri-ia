'use client'

import { useEffect, useRef } from 'react'

import { comporSaudacao, saudacaoDaHora } from '@/lib/saudacao'
import { enrollFace, whoIsThere } from '@/lib/tauri'
import { useChatStore, useSettingsStore } from '@/stores'

/**
 * A saudação de quando o app abre: ele olha pela webcam e chama você pelo nome.
 *
 * ## Por que espera as configurações
 *
 * A voz da persona vem do `settings.json`, e o `useBootstrap` dispara o carregamento sem
 * esperar. Falar antes disso é falar em silêncio: o `speakText` desiste quando não há voz
 * escolhida, e o efeito seria "às vezes ele saúda, às vezes não" — que é o tipo de bug
 * que ninguém consegue reproduzir.
 *
 * ## Por que uma trava de execução única
 *
 * Em `tauri dev` o hot reload remonta a página, e sem a trava a câmera acenderia a cada
 * salvamento de arquivo. A `ref` sobrevive ao re-render; o array de dependências, não.
 */
export function useSaudacao() {
  const isLoaded = useSettingsStore((state) => state.isLoaded)
  const anunciar = useChatStore((state) => state.anunciar)
  const jaSaudou = useRef(false)

  useEffect(() => {
    if (!isLoaded || jaSaudou.current) return
    jaSaudou.current = true

    void (async () => {
      let quem: Awaited<ReturnType<typeof whoIsThere>> | null = null
      try {
        quem = await whoIsThere()
      } catch {
        // Modelos não instalados, câmera ocupada por outro programa, sem permissão. Nada
        // disso merece um erro na cara de quem acabou de abrir o app — a saudação
        // simplesmente sai sem nome, que é o comportamento de quem nunca ligou isto.
      }

      await anunciar(comporSaudacao(saudacaoDaHora(new Date()), quem))
    })()
  }, [isLoaded, anunciar])
}

/**
 * Guarda o rosto de quem está na câmera, quando o agente pede.
 *
 * Separado do gancho da saudação porque o disparo é outro: aquele acontece no boot, este
 * quando alguém diz "eu sou o Guilherme" — e a mesma frase funciona horas depois, sem ter
 * havido pergunta nenhuma.
 */
export async function cadastrarRosto(nome: string): Promise<string> {
  try {
    const pessoa = await enrollFace(nome)

    // A contagem importa na resposta: a segunda foto é o que faz o reconhecimento
    // funcionar de noite depois de ter sido cadastrado de dia, e dizer isso convida a
    // repetir em vez de deixar o cadastro com um retrato só.
    return pessoa.cadastros > 1
      ? `Pronto, ${pessoa.nome} — já são ${pessoa.cadastros} registros do seu rosto.`
      : `Pronto, ${pessoa.nome}. Vou te reconhecer da próxima vez.`
  } catch (erro) {
    return erro instanceof Error ? erro.message : String(erro)
  }
}
