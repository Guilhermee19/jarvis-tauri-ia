'use client'

import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/Button'
import { Input } from '@/components/ui/Input'
import { listInputDevices, listWebcamResolutions } from '@/lib/tauri'
import {
  DEFAULT_SETTINGS,
  NOME_DA_PERSONA,
  type AppSettings,
  type Persona,
  type WebcamResolution,
} from '@/types'

/** `0×0` é o "automático" — o mesmo par que o Rust lê como `None`. */
const AUTOMATICO = '0x0'

function chave(width: number, height: number): string {
  return `${width}x${height}`
}

interface SettingsFormProps {
  initial: AppSettings
  isSaving: boolean
  onSubmit: (settings: AppSettings) => void
  onCancel: () => void
}

/**
 * O form é separado do modal de propósito: o `Modal` desmonta o conteúdo ao fechar,
 * então o estado inicial vem das props a cada abertura — sem efeito de sincronização,
 * e sem rascunho de uma edição abandonada aparecendo depois.
 */
export function SettingsForm({ initial, isSaving, onSubmit, onCancel }: SettingsFormProps) {
  const [apiKey, setApiKey] = useState(initial.anthropicApiKey)
  const [assistantName, setAssistantName] = useState(initial.assistantName)
  const [ollamaUrl, setOllamaUrl] = useState(initial.ollamaUrl)
  const [ollamaModel, setOllamaModel] = useState(initial.ollamaModel)
  const [memoriaPath, setMemoriaPath] = useState(initial.memoriaPath)
  const [braveKey, setBraveKey] = useState(initial.braveApiKey)
  const [spotifyId, setSpotifyId] = useState(initial.spotifyClientId)
  const [spotifySecret, setSpotifySecret] = useState(initial.spotifyClientSecret)
  const [tuyaId, setTuyaId] = useState(initial.tuyaClientId)
  const [tuyaSecret, setTuyaSecret] = useState(initial.tuyaClientSecret)
  const [tuyaRegiao, setTuyaRegiao] = useState(initial.tuyaRegiao)
  const [webcamResolucao, setWebcamResolucao] = useState(
    chave(initial.webcamWidth, initial.webcamHeight),
  )
  const [webcamMirror, setWebcamMirror] = useState(initial.webcamMirror)
  const [micDeviceName, setMicDeviceName] = useState(initial.micDeviceName)
  const [logDetalhado, setLogDetalhado] = useState(initial.logDetalhado)
  const [persona, setPersona] = useState(initial.persona)

  const { resolucoes, erro: erroResolucoes } = useWebcamResolutions()
  const { dispositivos, erro: erroDispositivos } = useInputDevices()

  /**
   * O nome ainda é o do tema (ou está vazio), então ele acompanha a troca.
   *
   * Sem esta checagem, escolher Ultron renomearia um assistente que a pessoa batizou de
   * "Sexta-feira" — e o gatilho de voz mudaria embaixo dela, sem aviso. Com ela, quem
   * nunca mexeu no campo tem o comportamento óbvio (o nome segue o tema), e quem
   * escolheu um nome fica com ele.
   */
  const nomeSegueOTema =
    assistantName.trim() === '' || Object.values(NOME_DA_PERSONA).includes(assistantName.trim())

  function trocarTema(proxima: Persona) {
    setPersona(proxima)
    if (nomeSegueOTema) setAssistantName(NOME_DA_PERSONA[proxima])
  }

  return (
    <div className="flex flex-col gap-4">
      <Input
        label="API key da Anthropic"
        type="password"
        value={apiKey}
        onChange={(event) => setApiKey(event.target.value)}
        placeholder="sk-ant-…"
        hint="Com ela, ele usa o Claude para OLHAR (tela e webcam) — identifica objeto e lê texto muito melhor que o modelo local, e custa ~US$ 0,01 por pergunta com imagem. Vazia, ele olha pelo Ollama, de graça. Guardada em texto puro no arquivo de config."
      />

      <Input
        label="Servidor do Ollama"
        value={ollamaUrl}
        onChange={(event) => setOllamaUrl(event.target.value)}
        placeholder={DEFAULT_SETTINGS.ollamaUrl}
        hint="Onde roda o intérprete de comandos. Precisa do Ollama instalado e ativo."
      />

      <Input
        label="Modelo do intérprete"
        value={ollamaModel}
        onChange={(event) => setOllamaModel(event.target.value)}
        placeholder={DEFAULT_SETTINGS.ollamaModel}
        hint="Baixe com `ollama pull qwen2.5vl:3b`. Precisa ser multimodal para o `o que é isso?` funcionar. Vazio desliga o intérprete."
      />

      <Input
        label="API key do Brave Search"
        type="password"
        value={braveKey}
        onChange={(event) => setBraveKey(event.target.value)}
        placeholder="BSA…"
        hint="Sem ela a busca usa a Wikipedia, que responde 'quem foi X' mas não sabe preço nem notícia. Grátis em brave.com/search/api (2000 buscas/mês)."
      />

      <Input
        label="Spotify — Client ID"
        value={spotifyId}
        onChange={(event) => setSpotifyId(event.target.value)}
        placeholder="sem isso, 'toque X' só abre a busca"
        hint="Crie um app em developer.spotify.com/dashboard (grátis, 2 min, sem login de usuário). É o que permite achar a faixa exata e tocar direto."
      />

      <Input
        label="Spotify — Client Secret"
        type="password"
        value={spotifySecret}
        onChange={(event) => setSpotifySecret(event.target.value)}
        placeholder="…"
      />

      <fieldset className="border-border-soft flex flex-col gap-3 rounded-lg border p-3">
        <legend className="text-muted px-1 text-xs font-medium">Áudio</legend>

        <label className="flex flex-col gap-1.5">
          <span className="text-muted text-xs font-medium">Microfone</span>
          <select
            value={micDeviceName}
            onChange={(event) => setMicDeviceName(event.target.value)}
            className="border-border-soft bg-base text-content focus:border-accent w-full rounded-lg border px-3 py-2 text-sm focus:outline-none"
          >
            <option value="">Padrão do sistema</option>
            {dispositivos.map((device) => (
              <option key={device} value={device}>
                {device}
              </option>
            ))}
          </select>
          <p className="text-muted text-[11px] leading-snug">
            {erroDispositivos
              ? `Não consegui listar os microfones (${erroDispositivos}). O padrão do sistema continua valendo.`
              : 'Vazio segue o padrão do Windows — que muda sozinho quando você pluga um fone ou liga a webcam, e é a razão mais comum de ele gravar do microfone errado. Escolher aqui trava num dispositivo, e a escolha vale só para o Jarvis.'}
          </p>
          {/* O nome é a única chave que temos para reencontrar o dispositivo, e ele some
              da lista quando você desconecta o fone. Sem este aviso, o `select` cairia em
              branco e a gravação falharia sem ninguém ligar uma coisa à outra. */}
          {micDeviceName !== '' &&
          dispositivos.length > 0 &&
          !dispositivos.includes(micDeviceName) ? (
            <p className="text-danger text-[11px] leading-snug">
              O microfone salvo (“{micDeviceName}”) não está conectado agora. A gravação vai falhar
              até ele voltar, ou até você escolher outro aqui.
            </p>
          ) : null}
        </label>
      </fieldset>

      <fieldset className="border-border-soft flex flex-col gap-3 rounded-lg border p-3">
        <legend className="text-muted px-1 text-xs font-medium">Casa inteligente (Tuya)</legend>

        <p className="text-muted text-[11px] leading-relaxed">
          Encontrar os aparelhos na rede não precisa de nada disto. Estas credenciais servem para
          buscar, <strong className="text-content font-normal">uma vez</strong>, o nome de cada um e
          a chave que permite mandar comando. Depois disso o controle é local, sem internet — e
          continua funcionando quando o projeto trial da Tuya expirar, porque a chave é do aparelho.
        </p>

        <Input
          label="Tuya — Access ID"
          value={tuyaId}
          onChange={(event) => setTuyaId(event.target.value)}
          placeholder="sem isto, a Casa só enxerga"
          hint="Crie um projeto Cloud em iot.tuya.com (Smart Home, grátis) e ligue a conta do app em Devices > Link App Account, lendo o QR code pelo Smart Life."
        />

        <Input
          label="Tuya — Access Secret"
          type="password"
          value={tuyaSecret}
          onChange={(event) => setTuyaSecret(event.target.value)}
          placeholder="…"
        />

        <label className="flex flex-col gap-1.5">
          <span className="text-muted text-xs font-medium">Tuya — Data center</span>
          <select
            value={tuyaRegiao}
            onChange={(event) => setTuyaRegiao(event.target.value)}
            className="border-border-soft bg-base text-content focus:border-accent rounded-md border px-2.5 py-2 text-xs outline-none"
          >
            <option value="us">América Ocidental (us)</option>
            <option value="eu">Europa Central (eu)</option>
            <option value="in">Índia (in)</option>
            <option value="cn">China (cn)</option>
          </select>
          {/* O campo que mais engana: errado, a Tuya responde SUCESSO com uma lista
              vazia em vez de recusar, e nada na resposta aponta para cá. */}
          <span className="text-muted text-[11px] leading-relaxed">
            Precisa ser o mesmo do projeto em iot.tuya.com. Se a importação disser que a conta não
            tem aparelho nenhum, é quase sempre isto — conta brasileira do Smart Life costuma ficar
            na América Ocidental.
          </span>
        </label>
      </fieldset>

      <fieldset className="border-border-soft flex flex-col gap-3 rounded-lg border p-3">
        <legend className="text-muted px-1 text-xs font-medium">Webcam</legend>

        <label className="flex flex-col gap-1.5">
          <span className="text-muted text-xs font-medium">Resolução</span>
          <select
            value={webcamResolucao}
            onChange={(event) => setWebcamResolucao(event.target.value)}
            className="border-border-soft bg-base text-content focus:border-accent w-full rounded-lg border px-3 py-2 text-sm focus:outline-none"
          >
            <option value={AUTOMATICO}>Automático (perto de 640×480)</option>
            {resolucoes.map((r) => (
              <option key={chave(r.width, r.height)} value={chave(r.width, r.height)}>
                {r.width}×{r.height} · até {r.maxFps} fps
              </option>
            ))}
          </select>
          <p className="text-muted text-[11px] leading-snug">
            {erroResolucoes
              ? `Não consegui perguntar à câmera (${erroResolucoes}). O automático continua valendo.`
              : 'A lista vem da própria câmera. A prévia é reduzida para o tamanho da janela, então resolução alta não a deixa lenta — ela vale para o que o modelo lê no “o que é isso?”. Vale saber que muitas webcams comprimem MJPEG mais forte em 1080p para caber na banda do USB, e aí 720p pode sair com menos artefato.'}
          </p>
          {/* O valor salvo pode não estar na lista — outra webcam, ou a mesma num
              modo diferente. Dizer isso é melhor que o `select` cair em branco. */}
          {webcamResolucao !== AUTOMATICO &&
          resolucoes.length > 0 &&
          !resolucoes.some((r) => chave(r.width, r.height) === webcamResolucao) ? (
            <p className="text-danger text-[11px] leading-snug">
              A câmera atual não oferece {webcamResolucao.replace('x', '×')}. Ela vai abrir na
              resolução mais próxima até você escolher outra.
            </p>
          ) : null}
        </label>

        <label className="flex cursor-pointer items-start gap-2">
          <input
            type="checkbox"
            checked={webcamMirror}
            onChange={(event) => setWebcamMirror(event.target.checked)}
            className="accent-accent border-border-soft bg-base mt-0.5 h-4 w-4 shrink-0 rounded border"
          />
          <span className="flex flex-col gap-0.5">
            <span className="text-content text-sm">Espelhar imagem</span>
            <span className="text-muted text-[11px] leading-snug">
              Visão de selfie: mover para a direita move para a direita na tela. Só muda a exibição
              — o quadro que vai para o modelo continua na orientação real.
            </span>
          </span>
        </label>
      </fieldset>

      <label className="flex cursor-pointer items-start gap-2">
        <input
          type="checkbox"
          checked={logDetalhado}
          onChange={(event) => setLogDetalhado(event.target.checked)}
          className="accent-accent border-border-soft bg-base mt-0.5 h-4 w-4 shrink-0 rounded border"
        />
        <span className="flex flex-col gap-0.5">
          <span className="text-content text-sm">Mostrar o log em toda mensagem</span>
          <span className="text-muted text-[11px] leading-snug">
            Normalmente o log só aparece quando ele executa algo ou mexe na memória. Ligado, aparece
            sempre — e mostra o VERBO que ele escolheu, que é o que revela quando ele entendeu seu
            pedido como conversa em vez de comando.
          </span>
        </span>
      </label>

      <Input
        label="Pasta da memória"
        value={memoriaPath}
        onChange={(event) => setMemoriaPath(event.target.value)}
        placeholder="memoria/ do projeto"
        hint="Onde ele guarda o que aprende, em markdown. Dá para abrir no Obsidian."
      />

      <Input
        label="Nome do assistente"
        value={assistantName}
        onChange={(event) => setAssistantName(event.target.value)}
        placeholder={DEFAULT_SETTINGS.assistantName}
        hint="É também o GATILHO de voz: dizer este nome antes da frase é o que a transforma em comando em vez de ditado."
      />

      <label className="flex flex-col gap-1.5">
        <span className="text-muted text-xs font-medium">Tema do sistema</span>
        <select
          value={persona}
          onChange={(event) => trocarTema(event.target.value as Persona)}
          className="border-border-soft bg-base text-content focus:border-accent w-full rounded-lg border px-3 py-2 text-sm focus:outline-none"
        >
          <option value="jarvis">Jarvis — azul, sóbrio e prestativo</option>
          <option value="ultron">Ultron — âmbar, seco e irônico</option>
        </select>
        <p className="text-muted text-[11px] leading-snug">
          Muda a cor do app, a voz e o jeito de falar. A cor troca na hora, sem reiniciar.
          {nomeSegueOTema
            ? ' O nome acima acompanha a troca — se você digitar um nome próprio, ele passa a mandar.'
            : ` O nome continua “${assistantName || DEFAULT_SETTINGS.assistantName}”, porque você escolheu um: apague o campo para ele voltar a seguir o tema.`}
        </p>
      </label>

      <div className="flex justify-end gap-2 pt-1">
        <Button variant="ghost" onClick={onCancel} disabled={isSaving}>
          Cancelar
        </Button>
        <Button
          onClick={() =>
            onSubmit({
              // `initial` primeiro para não apagar o que o form não edita — o clipe de
              // voz é escolhido no Diagnóstico, não aqui.
              ...initial,
              anthropicApiKey: apiKey.trim(),
              // Nome vazio cai no nome do TEMA escolhido, não no "Jarvis" fixo — senão
              // apagar o campo com o Ultron ativo devolveria o gatilho errado.
              assistantName: assistantName.trim() || NOME_DA_PERSONA[persona],
              persona,
              ollamaUrl: ollamaUrl.trim() || DEFAULT_SETTINGS.ollamaUrl,
              // Sem fallback aqui de propósito: vazio é uma escolha válida (desliga
              // o intérprete), diferente do nome e da URL.
              ollamaModel: ollamaModel.trim(),
              // Vazio também é válido: cai na pasta padrão.
              memoriaPath: memoriaPath.trim(),
              braveApiKey: braveKey.trim(),
              spotifyClientId: spotifyId.trim(),
              spotifyClientSecret: spotifySecret.trim(),
              tuyaClientId: tuyaId.trim(),
              tuyaClientSecret: tuyaSecret.trim(),
              tuyaRegiao,
              ...parseResolucao(webcamResolucao),
              webcamMirror,
              micDeviceName,
              logDetalhado,
            })
          }
          disabled={isSaving}
        >
          {isSaving ? 'Salvando…' : 'Salvar'}
        </Button>
      </div>
    </div>
  )
}

/** `"1280x720"` → `{ webcamWidth: 1280, webcamHeight: 720 }`. Lixo vira automático. */
function parseResolucao(valor: string): Pick<AppSettings, 'webcamWidth' | 'webcamHeight'> {
  const [largura, altura] = valor.split('x').map(Number)

  if (!Number.isFinite(largura) || !Number.isFinite(altura)) {
    return { webcamWidth: 0, webcamHeight: 0 }
  }

  return { webcamWidth: largura, webcamHeight: altura }
}

/**
 * Pergunta à câmera o que ela suporta, uma vez, ao abrir as configurações.
 *
 * Falhar aqui NÃO é erro de formulário: sem câmera conectada, ou com ela ocupada por
 * outro programa, o resto das configurações continua editável e o "automático" segue
 * sendo uma escolha válida. Por isso o erro vira texto de apoio, não um alerta.
 */
function useWebcamResolutions() {
  const [resolucoes, setResolucoes] = useState<WebcamResolution[]>([])
  const [erro, setErro] = useState<string | null>(null)

  useEffect(() => {
    let vivo = true

    listWebcamResolutions()
      .then((lista) => {
        if (vivo) setResolucoes(lista)
      })
      .catch((causa: unknown) => {
        if (vivo) setErro(causa instanceof Error ? causa.message : String(causa))
      })

    // O formulário some quando a gaveta fecha, e a resposta pode chegar depois.
    return () => {
      vivo = false
    }
  }, [])

  return { resolucoes, erro }
}

function useInputDevices() {
  const [dispositivos, setDispositivos] = useState<string[]>([])
  const [erro, setErro] = useState<string | null>(null)

  useEffect(() => {
    let vivo = true
    listInputDevices()
      .then((lista) => {
        if (vivo) setDispositivos(lista)
      })
      .catch((causa: unknown) => {
        if (vivo) setErro(causa instanceof Error ? causa.message : String(causa))
      })
    return () => {
      vivo = false
    }
  }, [])

  return { dispositivos, erro }
}
